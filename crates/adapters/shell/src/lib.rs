//! Owns bounded child processes: whole process-tree lifetime, output caps, timeouts and
//! cancellation, plus the shell runner used for goal checks, background jobs and keep-awake.
mod awake;
mod jobs;
use antiquota_core::{Cancellation, CommandOutcome, PortError, PortFuture, ShellRunner};
pub use awake::KeepAwake;
pub use jobs::Jobs;
use process_wrap::tokio::{CommandWrap, KillOnDrop};
use std::{process::Stdio, time::Duration};
use tokio::{io::AsyncReadExt, process::Command};

/// A finished (or ended) child: exit code, bounded output tails, and why it ended early.
#[derive(Debug, Default)]
pub struct Captured {
    pub code: Option<i32>,
    pub out: Vec<u8>,
    pub err: Vec<u8>,
    /// More output arrived than the limit; only its last bytes are kept.
    pub truncated: bool,
    pub timed_out: bool,
    pub cancelled: bool,
}

async fn tail(mut source: impl tokio::io::AsyncRead + Unpin, limit: usize) -> (Vec<u8>, bool) {
    let mut kept = Vec::new();
    let mut truncated = false;
    let mut chunk = [0u8; 65536];
    while let Ok(n) = source.read(&mut chunk).await {
        if n == 0 {
            break;
        }
        kept.extend_from_slice(&chunk[..n]);
        if kept.len() > limit {
            kept.drain(..kept.len() - limit);
            truncated = true;
        }
    }
    (kept, truncated)
}

/// Runs `command` in its own process group (a job object on Windows) and waits for it, its
/// output, a timeout or cancellation. Ending early kills the whole tree. stdin is closed.
pub async fn capture(
    mut command: Command,
    timeout: Duration,
    cancel: &Cancellation,
    limit: usize,
) -> Result<Captured, PortError> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut wrapped = CommandWrap::from(command);
    wrapped.wrap(KillOnDrop);
    #[cfg(unix)]
    wrapped.wrap(process_wrap::tokio::ProcessGroup::leader());
    #[cfg(windows)]
    wrapped.wrap(process_wrap::tokio::JobObject);
    let mut child = wrapped
        .spawn()
        .map_err(|e| PortError::Unavailable(format!("cannot start the command: {e}")))?;
    let stdout = child
        .stdout()
        .take()
        .ok_or_else(|| PortError::Unavailable("command output unavailable".into()))?;
    let stderr = child
        .stderr()
        .take()
        .ok_or_else(|| PortError::Unavailable("command error output unavailable".into()))?;
    let out = tokio::spawn(tail(stdout, limit));
    let err = tokio::spawn(tail(stderr, limit));
    let deadline = tokio::time::Instant::now() + timeout;
    let mut captured = Captured::default();
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|e| PortError::Unavailable(e.to_string()))?
        {
            captured.code = status.code();
            break;
        }
        if cancel.is_cancelled() || tokio::time::Instant::now() >= deadline {
            captured.cancelled = cancel.is_cancelled();
            captured.timed_out = !captured.cancelled;
            let _ = child.start_kill();
            let _ = child.wait().await;
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    // A descendant that inherited the pipes can keep them open; never wait on it for long.
    let _ = child.start_kill();
    let grace = Duration::from_millis(500);
    let (out, out_cut) = tokio::time::timeout(grace, out)
        .await
        .ok()
        .and_then(Result::ok)
        .unwrap_or_default();
    let (err, err_cut) = tokio::time::timeout(grace, err)
        .await
        .ok()
        .and_then(Result::ok)
        .unwrap_or_default();
    captured.out = out;
    captured.err = err;
    captured.truncated = out_cut || err_cut;
    Ok(captured)
}

/// The platform shell running one command line: `sh -c` on POSIX, `cmd /C` on Windows.
pub fn shell_command(line: &str) -> Command {
    if cfg!(windows) {
        let mut command = Command::new("cmd");
        command.args(["/C", line]);
        command
    } else {
        let mut command = Command::new("sh");
        command.args(["-c", line]);
        command
    }
}

/// Runs goal checks with the platform shell; output is stdout followed by stderr.
#[derive(Debug, Default, Clone)]
pub struct Shell;
impl ShellRunner for Shell {
    fn run<'a>(
        &'a self,
        cwd: &'a str,
        command: &'a str,
        timeout: f64,
        cancel: Cancellation,
    ) -> PortFuture<'a, CommandOutcome> {
        Box::pin(async move {
            let mut process = shell_command(command);
            process.current_dir(cwd);
            let captured = capture(
                process,
                Duration::from_secs_f64(timeout.max(0.0)),
                &cancel,
                1_000_000,
            )
            .await?;
            let mut output = String::from_utf8_lossy(&captured.out).into_owned();
            output.push_str(&String::from_utf8_lossy(&captured.err));
            Ok(CommandOutcome {
                code: if captured.timed_out || captured.cancelled {
                    None
                } else {
                    captured.code
                },
                output,
                timed_out: captured.timed_out,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn commands_report_exit_codes_output_timeouts_and_cancellation() {
        let dir = std::env::temp_dir();
        let cwd = dir.to_str().unwrap();
        let shell = Shell;
        let ok = shell
            .run(cwd, "echo out; echo err >&2", 10.0, Cancellation::default())
            .await
            .unwrap();
        assert_eq!(ok.code, Some(0));
        assert_eq!(ok.output, "out\nerr\n");
        let failed = shell
            .run(cwd, "exit 3", 10.0, Cancellation::default())
            .await
            .unwrap();
        assert_eq!(failed.code, Some(3));
        let started = std::time::Instant::now();
        let slow = shell
            .run(cwd, "sleep 30", 0.3, Cancellation::default())
            .await
            .unwrap();
        assert!(slow.timed_out && slow.code.is_none());
        assert!(started.elapsed() < Duration::from_secs(5));
        let cancel = Cancellation::default();
        let stopper = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            stopper.cancel();
        });
        let started = std::time::Instant::now();
        let stopped = shell
            .run(cwd, "sleep 30 & sleep 30", 30.0, cancel)
            .await
            .unwrap();
        assert!(!stopped.timed_out && stopped.code.is_none());
        assert!(started.elapsed() < Duration::from_secs(5));
    }
    #[tokio::test]
    async fn output_keeps_only_the_last_bytes_over_the_limit() {
        let captured = capture(
            shell_command("printf '%05000d' 0; printf END"),
            Duration::from_secs(10),
            &Cancellation::default(),
            100,
        )
        .await
        .unwrap();
        assert!(captured.truncated);
        assert_eq!(captured.out.len(), 100);
        assert!(captured.out.ends_with(b"END"));
    }
}
