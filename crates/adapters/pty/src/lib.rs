//! Owns a bounded portable PTY/ConPTY console for agy's authorization-code input.
use antiquota_core::PortError;
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
pub struct Console {
    child: Mutex<Box<dyn Child + Send + Sync>>,
    master: Mutex<Option<Box<dyn MasterPty + Send>>>,
    writer: Mutex<Option<Box<dyn Write + Send>>>,
    output: Arc<Mutex<Vec<u8>>>,
    eof: Arc<AtomicBool>,
    stopped: AtomicBool,
    #[cfg(unix)]
    pid: Option<u32>,
    #[cfg(windows)]
    job: Mutex<Option<win32job::Job>>,
}
fn failure(e: impl std::fmt::Display) -> PortError {
    PortError::Unavailable(format!("sign-in console: {e}"))
}
impl Console {
    pub fn spawn(
        executable: &Path,
        args: &[String],
        cwd: &Path,
        environment: &BTreeMap<String, String>,
    ) -> Result<Arc<Self>, PortError> {
        // Official portable-pty API supplies native PTY on Unix and ConPTY on Windows.
        // https://docs.rs/portable-pty/0.9.0/portable_pty/
        // Agy prints long OAuth URLs and JSON; match its legacy unwrapped console.
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: 24,
                cols: 8000,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(failure)?;
        let mut command = CommandBuilder::new(executable);
        command.args(args);
        command.cwd(cwd);
        for (key, value) in environment {
            command.env(key, value);
        }
        // Acquire pipes before spawning so pipe failures cannot orphan a child.
        let reader = pair.master.try_clone_reader().map_err(failure)?;
        let writer = pair.master.take_writer().map_err(failure)?;
        #[allow(unused_mut)]
        let mut child = pair.slave.spawn_command(command).map_err(failure)?;
        drop(pair.slave);
        #[cfg(unix)]
        let pid = child.process_id();
        #[cfg(windows)]
        let job = {
            // https://docs.rs/win32job/2.0.3/win32job/struct.Job.html
            let mut limits = win32job::ExtendedLimitInfo::new();
            limits.limit_kill_on_job_close();
            let assigned = (|| {
                let job = win32job::Job::create_with_limit_info(&limits).map_err(failure)?;
                let handle = child
                    .as_raw_handle()
                    .ok_or_else(|| failure("missing process handle"))?;
                job.assign_process(handle as isize).map_err(failure)?;
                Ok::<_, PortError>(job)
            })();
            match assigned {
                Ok(job) => job,
                Err(e) => {
                    let _ = child.kill();
                    return Err(e);
                }
            }
        };
        let output = Arc::new(Mutex::new(vec![]));
        let eof = Arc::new(AtomicBool::new(false));
        let console = Arc::new(Self {
            child: Mutex::new(child),
            master: Mutex::new(Some(pair.master)),
            writer: Mutex::new(Some(writer)),
            output: output.clone(),
            eof: eof.clone(),
            stopped: AtomicBool::new(false),
            #[cfg(unix)]
            pid,
            #[cfg(windows)]
            job: Mutex::new(Some(job)),
        });
        thread::spawn(move || {
            let mut reader = reader;
            let mut chunk = [0; 8192];
            while let Ok(count) = reader.read(&mut chunk) {
                if count == 0 {
                    break;
                }
                if let Ok(mut output) = output.lock() {
                    output.extend_from_slice(&chunk[..count]);
                    if output.len() > 1_000_000 {
                        let excess = output.len() - 1_000_000;
                        output.drain(..excess);
                    }
                } else {
                    break;
                }
            }
            eof.store(true, Ordering::SeqCst);
        });
        Ok(console)
    }
    pub fn text(&self) -> String {
        self.output
            .lock()
            .map(|v| String::from_utf8_lossy(&v).into_owned())
            .unwrap_or_default()
    }
    pub fn write(&self, text: &str) -> Result<(), PortError> {
        let mut writer = self.writer.lock().map_err(failure)?;
        let writer = writer
            .as_mut()
            .ok_or_else(|| failure("sign-in already finished"))?;
        writer.write_all(text.as_bytes()).map_err(failure)?;
        writer.flush().map_err(failure)
    }
    pub fn finished(&self) -> Result<bool, PortError> {
        let done = self
            .child
            .lock()
            .map_err(failure)?
            .try_wait()
            .map_err(failure)?
            .is_some();
        if done {
            self.stop();
        }
        Ok(done)
    }
    pub fn stop(&self) {
        if self.stopped.swap(true, Ordering::SeqCst) {
            return;
        }
        #[cfg(unix)]
        if let Some(pid) = self.pid.and_then(|v| i32::try_from(v).ok()) {
            // portable-pty makes the child a session/group leader before exec.
            // https://docs.rs/portable-pty/0.9.0/src/portable_pty/unix.rs.html
            let _ = nix::sys::signal::killpg(
                nix::unistd::Pid::from_raw(pid),
                nix::sys::signal::Signal::SIGKILL,
            );
        }
        #[cfg(windows)]
        if let Ok(mut job) = self.job.lock() {
            job.take();
        }
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
        }
        if let Ok(mut writer) = self.writer.lock() {
            writer.take();
        }
        if let Ok(mut master) = self.master.lock() {
            master.take();
        }
    }
    pub fn drained(&self) -> bool {
        self.eof.load(Ordering::SeqCst)
    }
    pub fn reap(&self) {
        let until = Instant::now() + Duration::from_secs(2);
        while Instant::now() < until {
            if self
                .child
                .lock()
                .ok()
                .and_then(|mut c| c.try_wait().ok())
                .flatten()
                .is_some()
            {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}
impl Drop for Console {
    fn drop(&mut self) {
        self.stop();
        self.reap();
    }
}
