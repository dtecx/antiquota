//! Owns bounded stdout framing, stderr tails, and cancellable pipe readers.
use antiquota_core::PortError;
use std::time::Duration;
use tokio::{
    io::AsyncReadExt,
    process::{ChildStderr, ChildStdout},
    sync::mpsc,
    task::JoinHandle,
};
pub struct Output {
    pub lines: mpsc::Receiver<Result<Vec<u8>, PortError>>,
    stdout: JoinHandle<()>,
    stderr: JoinHandle<Vec<u8>>,
}
impl Output {
    pub fn read(stdout: ChildStdout, stderr: ChildStderr) -> Self {
        let stderr = tokio::spawn(async move {
            let mut reader = stderr;
            let mut tail = vec![];
            let mut chunk = [0u8; 8192];
            while let Ok(count) = reader.read(&mut chunk).await {
                if count == 0 {
                    break;
                }
                tail.extend_from_slice(&chunk[..count]);
                if tail.len() > 65536 {
                    tail.drain(..tail.len() - 65536);
                }
            }
            tail
        });
        let (sender, lines) = mpsc::channel(32);
        let stdout = tokio::spawn(async move {
            let mut reader = stdout;
            let mut pending = vec![];
            let mut chunk = [0u8; 8192];
            loop {
                let count = match reader.read(&mut chunk).await {
                    Ok(count) => count,
                    Err(e) => {
                        let _ = sender
                            .send(Err(PortError::Unavailable(e.to_string())))
                            .await;
                        return;
                    }
                };
                if count == 0 {
                    if !pending.is_empty() {
                        let _ = sender.send(Ok(pending)).await;
                    }
                    return;
                }
                for byte in &chunk[..count] {
                    pending.push(*byte);
                    if pending.len() > 4_000_000 {
                        let _ = sender
                            .send(Err(PortError::Invalid(
                                "agy protocol line exceeds 4 MB".into(),
                            )))
                            .await;
                        return;
                    }
                    if *byte == b'\n'
                        && sender.send(Ok(std::mem::take(&mut pending))).await.is_err()
                    {
                        return;
                    }
                }
            }
        });
        Self {
            lines,
            stdout,
            stderr,
        }
    }
    pub async fn finish(&mut self) -> Vec<u8> {
        self.stdout.abort();
        match tokio::time::timeout(Duration::from_millis(500), &mut self.stderr).await {
            Ok(Ok(bytes)) => bytes,
            _ => {
                self.stderr.abort();
                vec![]
            }
        }
    }
}
impl Drop for Output {
    fn drop(&mut self) {
        self.stdout.abort();
        self.stderr.abort();
    }
}
