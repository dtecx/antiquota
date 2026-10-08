//! Owns the Terminal panel's shells: a login shell in a pseudo-terminal (ConPTY on Windows),
//! a bounded replay buffer for late subscribers, and bounded per-subscriber queues.
use antiquota_core::{PortError, Terminal, TerminalChunk, TerminalFeed, TerminalInfo, Terminals};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::{
    io::{Read, Write},
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{SyncSender, TrySendError, sync_channel},
    },
    thread,
};

/// Bytes of recent output a new subscriber receives.
const KEEP: usize = 256 * 1024;
/// Chunks a slow subscriber may fall behind before newer output is dropped for it.
const QUEUE: usize = 4000;
const EXITED: &[u8] = b"\r\n\x1b[2m[process exited]\x1b[0m\r\n";

fn failure(e: impl std::fmt::Display) -> PortError {
    PortError::Unavailable(format!("terminal: {e}"))
}

#[derive(Default)]
struct Shared {
    buffer: Vec<u8>,
    subscribers: Vec<SyncSender<TerminalChunk>>,
}

pub struct ShellTerminal {
    info: Mutex<TerminalInfo>,
    alive: Arc<AtomicBool>,
    shared: Arc<Mutex<Shared>>,
    child: Mutex<Box<dyn Child + Send + Sync>>,
    master: Mutex<Option<Box<dyn MasterPty + Send>>>,
    writer: Mutex<Option<Box<dyn Write + Send>>>,
    #[cfg(unix)]
    pid: Option<u32>,
    #[cfg(windows)]
    job: Mutex<Option<win32job::Job>>,
}

fn emit(shared: &Mutex<Shared>, data: &[u8]) {
    let Ok(mut shared) = shared.lock() else {
        return;
    };
    shared.buffer.extend_from_slice(data);
    if shared.buffer.len() > KEEP {
        let excess = shared.buffer.len() - KEEP;
        shared.buffer.drain(..excess);
    }
    shared.subscribers.retain(|tx| {
        // A full queue drops this chunk for that subscriber; a closed one is removed.
        !matches!(
            tx.try_send(TerminalChunk::Data(data.to_vec())),
            Err(TrySendError::Disconnected(_))
        )
    });
}

fn ended(shared: &Mutex<Shared>, alive: &AtomicBool) {
    alive.store(false, Ordering::SeqCst);
    emit(shared, EXITED);
    let subscribers = shared
        .lock()
        .map(|mut s| std::mem::take(&mut s.subscribers))
        .unwrap_or_default();
    for tx in subscribers {
        // The exit must arrive even after a lagging subscriber's queue filled up.
        if let Err(TrySendError::Full(chunk)) = tx.try_send(TerminalChunk::Exit) {
            thread::spawn(move || {
                let _ = tx.send(chunk);
            });
        }
    }
}

impl ShellTerminal {
    pub fn spawn(cwd: &str, cols: u16, rows: u16) -> Result<Arc<Self>, PortError> {
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: rows.max(2),
                cols: cols.max(2),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(failure)?;
        let (program, shell) = if cfg!(windows) {
            let program = std::env::var("COMSPEC").unwrap_or_else(|_| "powershell.exe".into());
            let name = Path::new(&program)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| program.clone());
            (program, name)
        } else {
            let program = std::env::var("SHELL")
                .ok()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "/bin/zsh".into());
            let name = Path::new(&program)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| program.clone());
            (program, name)
        };
        let mut command = CommandBuilder::new(&program);
        if !cfg!(windows) {
            command.arg("-l");
        }
        command.cwd(cwd);
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        let reader = pair.master.try_clone_reader().map_err(failure)?;
        let writer = pair.master.take_writer().map_err(failure)?;
        #[allow(unused_mut)]
        let mut child = pair.slave.spawn_command(command).map_err(failure)?;
        drop(pair.slave);
        #[cfg(unix)]
        let pid = child.process_id();
        #[cfg(windows)]
        let job = {
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
        let alive = Arc::new(AtomicBool::new(true));
        let shared = Arc::new(Mutex::new(Shared::default()));
        let terminal = Arc::new(Self {
            info: Mutex::new(TerminalInfo {
                alive: true,
                shell,
                cwd: cwd.into(),
            }),
            alive: alive.clone(),
            shared: shared.clone(),
            child: Mutex::new(child),
            master: Mutex::new(Some(pair.master)),
            writer: Mutex::new(Some(writer)),
            #[cfg(unix)]
            pid,
            #[cfg(windows)]
            job: Mutex::new(Some(job)),
        });
        thread::spawn(move || {
            let mut reader = reader;
            let mut chunk = [0u8; 65536];
            while let Ok(count) = reader.read(&mut chunk) {
                if count == 0 {
                    break;
                }
                emit(&shared, &chunk[..count]);
            }
            ended(&shared, &alive);
        });
        Ok(terminal)
    }
}

impl Terminal for ShellTerminal {
    fn info(&self) -> TerminalInfo {
        let mut info = self.info.lock().map(|i| i.clone()).unwrap_or(TerminalInfo {
            alive: false,
            shell: String::new(),
            cwd: String::new(),
        });
        info.alive = self.alive.load(Ordering::SeqCst);
        info
    }

    fn write(&self, data: &[u8]) -> Result<(), PortError> {
        if !self.alive.load(Ordering::SeqCst) {
            return Err(PortError::Invalid(
                "the shell has exited: restart it".into(),
            ));
        }
        let mut writer = self.writer.lock().map_err(failure)?;
        let writer = writer
            .as_mut()
            .ok_or_else(|| PortError::Invalid("the shell has exited: restart it".into()))?;
        writer.write_all(data).map_err(failure)?;
        writer.flush().map_err(failure)
    }

    fn resize(&self, cols: u16, rows: u16) -> Result<(), PortError> {
        if !self.alive.load(Ordering::SeqCst) {
            return Ok(());
        }
        if let Some(master) = self.master.lock().map_err(failure)?.as_ref() {
            master
                .resize(PtySize {
                    rows: rows.max(2),
                    cols: cols.max(2),
                    pixel_width: 0,
                    pixel_height: 0,
                })
                .map_err(failure)?;
        }
        Ok(())
    }

    fn kill(&self) {
        #[cfg(unix)]
        if let Some(pid) = self.pid.and_then(|v| i32::try_from(v).ok()) {
            let _ = nix::sys::signal::killpg(
                nix::unistd::Pid::from_raw(pid),
                nix::sys::signal::Signal::SIGHUP,
            );
        }
        #[cfg(windows)]
        if let Ok(mut job) = self.job.lock() {
            job.take();
        }
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.try_wait();
        }
        if let Ok(mut writer) = self.writer.lock() {
            writer.take();
        }
    }

    fn subscribe(&self) -> TerminalFeed {
        let (tx, rx) = sync_channel(QUEUE);
        let recent = match self.shared.lock() {
            Ok(mut shared) => {
                if self.alive.load(Ordering::SeqCst) {
                    shared.subscribers.push(tx);
                } else {
                    let _ = tx.try_send(TerminalChunk::Exit);
                }
                shared.buffer.clone()
            }
            Err(_) => {
                let _ = tx.try_send(TerminalChunk::Exit);
                vec![]
            }
        };
        TerminalFeed { recent, chunks: rx }
    }
}

impl Drop for ShellTerminal {
    fn drop(&mut self) {
        self.kill();
    }
}

/// Starts login shells in pseudo-terminals.
#[derive(Debug, Default, Clone)]
pub struct Shells;
impl Terminals for Shells {
    fn spawn(&self, cwd: &str, cols: u16, rows: u16) -> Result<Arc<dyn Terminal>, PortError> {
        Ok(ShellTerminal::spawn(cwd, cols, rows)?)
    }
}

// Both shell tests need a POSIX shell, so the helpers are POSIX-only too.
#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn collect_until(feed: &TerminalFeed, needle: &str, seconds: u64) -> (String, bool) {
        let mut text = String::from_utf8_lossy(&feed.recent).into_owned();
        let until = Instant::now() + Duration::from_secs(seconds);
        let mut exited = false;
        while !text.contains(needle) && Instant::now() < until {
            match feed.chunks.recv_timeout(Duration::from_millis(100)) {
                Ok(TerminalChunk::Data(bytes)) => text.push_str(&String::from_utf8_lossy(&bytes)),
                Ok(TerminalChunk::Exit) => {
                    exited = true;
                    break;
                }
                Err(_) => {}
            }
        }
        (text, exited)
    }

    #[test]
    #[cfg(unix)]
    fn a_shell_runs_in_its_folder_replays_output_and_ends() {
        let dir = std::env::temp_dir().canonicalize().unwrap();
        let cwd = dir.to_str().unwrap();
        let terminal = ShellTerminal::spawn(cwd, 80, 24).unwrap();
        let info = terminal.info();
        assert!(info.alive);
        assert_eq!(info.cwd, cwd);
        assert!(!info.shell.is_empty());
        let feed = terminal.subscribe();
        terminal
            .write(b"printf 'from-%s\\n' terminal; pwd\r")
            .unwrap();
        let (text, _) = collect_until(&feed, "from-terminal", 10);
        assert!(text.contains("from-terminal"), "{text}");
        terminal.resize(1, 1).unwrap();
        let late = terminal.subscribe();
        assert!(
            String::from_utf8_lossy(&late.recent).contains("from-terminal"),
            "late subscribers see recent output"
        );
        terminal.write(b"exit\r").unwrap();
        let (text, exited) = collect_until(&feed, "[process exited]", 10);
        assert!(text.contains("[process exited]"));
        let until = Instant::now() + Duration::from_secs(5);
        while !exited && terminal.info().alive && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(!terminal.info().alive);
        assert_eq!(
            terminal.write(b"x").unwrap_err().to_string(),
            "the shell has exited: restart it"
        );
        terminal.resize(80, 24).unwrap();
        let after = terminal.subscribe();
        assert_eq!(
            after.chunks.recv_timeout(Duration::from_secs(1)),
            Ok(TerminalChunk::Exit)
        );
        terminal.kill();
        terminal.kill();
    }

    #[test]
    #[cfg(unix)]
    fn kill_ends_the_shell_and_its_children() {
        let dir = std::env::temp_dir().canonicalize().unwrap();
        let terminal = ShellTerminal::spawn(dir.to_str().unwrap(), 80, 24).unwrap();
        let feed = terminal.subscribe();
        terminal.write(b"sleep 60 &\r").unwrap();
        std::thread::sleep(Duration::from_millis(300));
        terminal.kill();
        let (_, exited) = collect_until(&feed, "never-printed", 8);
        assert!(exited || !terminal.info().alive);
    }
}
