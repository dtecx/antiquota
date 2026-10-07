//! Owns isolated agy subprocesses and current-turn stream decoding.
mod file_diff;
mod output;
mod parser;
mod preflight;
mod process_tree;
use antiquota_core::{
    AgentRunner, Cancellation, PortError, PortFuture, TurnRequest, TurnResult, TurnSink,
};
pub use parser::Decoder;
pub use preflight::isolated_fake;
use std::{path::PathBuf, process::Stdio};
use tokio::process::Command;
#[derive(Debug, Clone)]
pub struct AgyRunner {
    pub executable: PathBuf,
    pub hook_program: PathBuf,
    pub isolated_fake: bool,
}
impl AgentRunner for AgyRunner {
    fn run<'a>(
        &'a self,
        request: TurnRequest,
        cancel: Cancellation,
        mut sink: Box<dyn TurnSink + 'a>,
    ) -> PortFuture<'a, TurnResult> {
        Box::pin(async move {
            self.preflight(&request).await?;
            let mut command = Command::new(&self.executable);
            command.args([
                "-p",
                &request.prompt,
                "--output-format",
                "stream-json",
                "--model",
                &request.model,
                "--dangerously-skip-permissions",
            ]);
            if let Some(conv) = &request.conversation_id {
                command.args(["--conversation", conv]);
            }
            if matches!(request.mode, antiquota_core::Mode::AcceptEdits) {
                command.args(["--mode", "accept-edits"]);
            }
            command
                .current_dir(&request.cwd)
                .env("HOME", &request.slot.home)
                .env("USERPROFILE", &request.slot.home)
                .env("SSH_CONNECTION", "127.0.0.1 1 127.0.0.1 1")
                .env("ANTIQUOTA_SLOT", &request.slot.name)
                .env("ANTIQUOTA_SESSION", &request.session)
                .env("ANTIQUOTA_STATE", &request.state_dir)
                .env(
                    "ANTIQUOTA_MODE",
                    serde_json::to_value(request.mode)
                        .map_err(|e| PortError::Invalid(e.to_string()))?
                        .as_str()
                        .unwrap_or("default"),
                )
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true);
            let mut tree = process_tree::ProcessTree::spawn(command)?;
            let stdout = tree
                .child
                .stdout()
                .take()
                .ok_or_else(|| PortError::Unavailable("agy stdout unavailable".into()))?;
            let stderr = tree
                .child
                .stderr()
                .take()
                .ok_or_else(|| PortError::Unavailable("agy stderr unavailable".into()))?;
            let mut output = output::Output::read(stdout, stderr);
            let mut decoder = Decoder::new(request.turn);
            let mut file_diff = file_diff::FileDiff::new(PathBuf::from(&request.cwd));
            let mut reported_conversation = request.conversation_id.clone();
            let mut status = None;
            let mut draining = None;
            let processing: Result<(), PortError> = async {
                loop {
                    if cancel.is_cancelled() {
                        tree.stop();
                        break;
                    }
                    if status.is_none()
                        && let Some(exited) = tree.status()?
                    {
                        status = Some(exited);
                        tree.stop();
                        draining = Some(tokio::time::Instant::now());
                    }
                    if draining.is_some_and(|started| {
                        started.elapsed() > std::time::Duration::from_secs(1)
                    }) {
                        break;
                    }
                    let next = tokio::select! {
                        result = output.lines.recv() => Some(result),
                        _ = tokio::time::sleep(std::time::Duration::from_millis(20)) => None,
                    };
                    let Some(next) = next else { continue };
                    let Some(line) = next else { break };
                    if let Ok(value) = serde_json::from_slice(&line?) {
                        let events = decoder.accept(value)?;
                        if let Some(conversation) =
                            decoder.conversation_id().filter(|id| !id.is_empty())
                            && reported_conversation.as_deref() != Some(conversation)
                        {
                            sink.conversation_id(conversation)?;
                            reported_conversation = Some(conversation.into());
                        }
                        for mut event in events {
                            file_diff.decorate(&mut event);
                            sink.event(event)?;
                        }
                    }
                }
                Ok(())
            }
            .await;
            let result = tree.finish(status).await;
            let stderr = output.finish().await;
            processing?;
            let status = result?;
            Ok(decoder.finish(
                status.code().unwrap_or(-1),
                cancel.is_cancelled(),
                &String::from_utf8_lossy(&stderr),
            ))
        })
    }
}
