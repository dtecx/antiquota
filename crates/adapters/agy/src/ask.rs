//! Owns separate read-only agy calls in a fresh conversation, such as the goal verifier.
use crate::AgyRunner;
use antiquota_core::{AskRequest, AskResult, Cancellation, Failure, PortError, classify_error};
use std::time::Duration;
use tokio::process::Command;

/// The longest a verifier call may take before it counts as failed.
const ASK_TIMEOUT: Duration = Duration::from_secs(1800);
const OUTPUT_LIMIT: usize = 4_000_000;

impl AgyRunner {
    /// Runs `agy -p … --output-format json` in the project folder on the leased account. agy's
    /// own permission mode applies and no session variables are passed, so antiquota's hook
    /// leaves the call to agy: the verifier can only view and list files.
    pub(crate) async fn ask_agy(
        &self,
        request: AskRequest,
        cancel: Cancellation,
    ) -> Result<AskResult, PortError> {
        self.ensure_executable()?;
        self.token_ready(&request.slot.home)?;
        let mut command = Command::new(&self.executable);
        command
            .args([
                "-p",
                &request.prompt,
                "--output-format",
                "json",
                "--model",
                &request.model,
            ])
            .current_dir(&request.cwd)
            .env("HOME", &request.slot.home)
            .env("USERPROFILE", &request.slot.home)
            .env("SSH_CONNECTION", "127.0.0.1 0 127.0.0.1 0")
            .env("ANTIQUOTA_SLOT", &request.slot.name)
            .env_remove("ANTIQUOTA_SESSION")
            .env_remove("ANTIQUOTA_STATE")
            .env_remove("ANTIQUOTA_MODE");
        let captured =
            antiquota_shell::capture(command, ASK_TIMEOUT, &cancel, OUTPUT_LIMIT).await?;
        let out = String::from_utf8_lossy(&captured.out).into_owned();
        let err = String::from_utf8_lossy(&captured.err).into_owned();
        if captured.cancelled {
            return Ok(AskResult {
                status: "stopped".into(),
                ..Default::default()
            });
        }
        if captured.timed_out {
            return Ok(AskResult {
                status: "error".into(),
                error: "agy did not answer in time".into(),
                ..Default::default()
            });
        }
        let value: Option<serde_json::Value> = (!captured.truncated)
            .then(|| serde_json::from_str(out.trim()).ok())
            .flatten();
        let response = value
            .as_ref()
            .and_then(|v| v["response"].as_str())
            .map(str::to_owned);
        let failed = captured.code != Some(0)
            || value.as_ref().is_some_and(|v| v["status"] == "ERROR")
            || response.is_none();
        if failed {
            let text = format!("{err}\n{out}");
            let status = match classify_error(&text) {
                Some(Failure::Quota) => "quota",
                Some(Failure::Auth) => "auth",
                _ => "error",
            };
            let tail: String = text
                .trim()
                .chars()
                .rev()
                .take(600)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            return Ok(AskResult {
                status: status.into(),
                response: response.unwrap_or_default(),
                error: tail,
            });
        }
        Ok(AskResult {
            status: "SUCCESS".into(),
            response: response.unwrap_or_default(),
            error: String::new(),
        })
    }
}
