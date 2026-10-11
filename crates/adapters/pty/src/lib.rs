//! Owns portable PTY/ConPTY consoles: agy's authorization-code input and the Terminal panel.
mod terminal;
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
pub use terminal::{ShellTerminal, Shells};
pub struct Console {
    child: Mutex<Box<dyn Child + Send + Sync>>,
    master: Mutex<Option<Box<dyn MasterPty + Send>>>,
    writer: Arc<Mutex<Option<Box<dyn Write + Send>>>>,
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
/// The cursor-position question a console asks its terminal.
const CURSOR_QUERY: &[u8] = b"\x1b[6n";
#[derive(Default)]
struct CursorQueries {
    pending: Vec<u8>,
}
impl CursorQueries {
    fn without_cursor_queries(&mut self, bytes: &[u8]) -> (Vec<u8>, usize) {
        self.pending.extend_from_slice(bytes);
        let mut rest = Vec::with_capacity(self.pending.len());
        let mut questions = 0;
        let mut at = 0;
        while at < self.pending.len() {
            if self.pending[at..].starts_with(CURSOR_QUERY) {
                at += CURSOR_QUERY.len();
                questions += 1;
            } else if CURSOR_QUERY.starts_with(&self.pending[at..]) {
                // A read may end inside the question; keep only that possible prefix.
                break;
            } else {
                rest.push(self.pending[at]);
                at += 1;
            }
        }
        self.pending.drain(..at);
        (rest, questions)
    }
    /// ConPTY waits for a cursor answer before starting. Answer without a viewer and
    /// remove the question from output so an attached viewer never answers it twice.
    fn answer(&mut self, bytes: &[u8], writer: &Mutex<Option<Box<dyn Write + Send>>>) -> Vec<u8> {
        if !cfg!(windows) {
            return bytes.to_vec();
        }
        let (rest, questions) = self.without_cursor_queries(bytes);
        if questions > 0
            && let Ok(mut writer) = writer.lock()
            && let Some(writer) = writer.as_mut()
        {
            for _ in 0..questions {
                let _ = writer.write_all(b"\x1b[1;1R");
            }
            let _ = writer.flush();
        }
        rest
    }
    fn finish(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.pending)
    }
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
        let writer = Arc::new(Mutex::new(Some(writer)));
        let answers = writer.clone();
        let console = Arc::new(Self {
            child: Mutex::new(child),
            master: Mutex::new(Some(pair.master)),
            writer,
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
            let mut queries = CursorQueries::default();
            loop {
                let (data, done) = match reader.read(&mut chunk) {
                    Ok(0) | Err(_) => (queries.finish(), true),
                    Ok(count) => (queries.answer(&chunk[..count], &answers), false),
                };
                if let Ok(mut output) = output.lock() {
                    output.extend_from_slice(&data);
                    if output.len() > 1_000_000 {
                        let excess = output.len() - 1_000_000;
                        output.drain(..excess);
                    }
                } else {
                    break;
                }
                if done {
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

#[cfg(test)]
mod tests {
    use super::{CURSOR_QUERY, CursorQueries};

    fn filtered(chunks: &[&[u8]]) -> (Vec<u8>, usize) {
        let mut queries = CursorQueries::default();
        let mut output = Vec::new();
        let mut questions = 0;
        for chunk in chunks {
            let (bytes, count) = queries.without_cursor_queries(chunk);
            output.extend(bytes);
            questions += count;
            assert!(queries.pending.len() < CURSOR_QUERY.len());
        }
        output.extend(queries.finish());
        assert!(queries.finish().is_empty());
        (output, questions)
    }

    #[test]
    fn cursor_questions_are_found_and_dropped_from_output() {
        assert_eq!(filtered(&[b"plain output"]), (b"plain output".to_vec(), 0));
        assert_eq!(
            filtered(&[b"\x1b[6nPS C:\\> \x1b[6n"]),
            (b"PS C:\\> ".to_vec(), 2)
        );
    }

    #[test]
    fn cursor_questions_survive_every_split_and_adjacent_questions() {
        let bytes = b"before\x1b[6n\x1b[6nmiddle\x1b[6nafter";
        for split in 0..=bytes.len() {
            assert_eq!(
                filtered(&[&bytes[..split], &bytes[split..]]),
                (b"beforemiddleafter".to_vec(), 3),
                "split at {split}"
            );
        }
        let chunks: Vec<_> = bytes.chunks(1).collect();
        assert_eq!(filtered(&chunks), (b"beforemiddleafter".to_vec(), 3));
    }

    #[test]
    fn incomplete_and_non_query_bytes_are_preserved() {
        let bytes = b"\xff\x1b[31m\x1b[6x\x1b\x1b[6\r\n";
        for split in 0..=bytes.len() {
            assert_eq!(
                filtered(&[&bytes[..split], &bytes[split..]]),
                (bytes.to_vec(), 0),
                "split at {split}"
            );
        }
        for len in 1..CURSOR_QUERY.len() {
            assert_eq!(
                filtered(&[b"before", &CURSOR_QUERY[..len]]),
                ([b"before".as_slice(), &CURSOR_QUERY[..len]].concat(), 0)
            );
        }
    }

    #[test]
    #[cfg(not(windows))]
    fn other_platforms_pass_cursor_questions_through_without_buffering() {
        let mut queries = CursorQueries::default();
        let writer = std::sync::Mutex::new(None);
        for byte in CURSOR_QUERY.chunks(1) {
            assert_eq!(queries.answer(byte, &writer), byte);
        }
        assert!(queries.finish().is_empty());
    }
}
