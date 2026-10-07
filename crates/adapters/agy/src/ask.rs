//! Owns separate read-only agy calls in a fresh conversation, such as the goal verifier.
use crate::AgyRunner;
use antiquota_core::{AskRequest, AskResult, Cancellation, Failure, PortError, classify_error};
use std::time::Duration;
use tokio::process::Command;

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
            .args(
                request
                    .add_dirs
                    .iter()
                    .flat_map(|d| ["--add-dir".to_owned(), d.clone()]),
            )
            .current_dir(&request.cwd)
            .env("HOME", &request.slot.home)
            .env("USERPROFILE", &request.slot.home)
            .env("SSH_CONNECTION", "127.0.0.1 0 127.0.0.1 0")
            .env("ANTIQUOTA_SLOT", &request.slot.name)
            .env_remove("ANTIQUOTA_SESSION")
            .env_remove("ANTIQUOTA_STATE")
            .env_remove("ANTIQUOTA_MODE");
        let timeout = Duration::from_secs_f64(request.timeout.clamp(1.0, 3600.0));
        let captured = antiquota_shell::capture(command, timeout, &cancel, OUTPUT_LIMIT).await?;
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

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use antiquota_core::Slot;
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
    };

    /// A stand-in agy that records its arguments and prints `output`.
    fn stub(root: &Path, output: &str) -> AgyRunner {
        let script = root.join("agy");
        fs::write(
            &script,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"{}\"\nprintf '%s' '{output}'\n",
                root.join("argv").display()
            ),
        )
        .unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();
        AgyRunner {
            executable: script,
            hook_program: "not-executed".into(),
            isolated_fake: true,
        }
    }

    #[tokio::test]
    async fn answers_need_agy_json_and_pass_folder_access_without_skipping_permissions() {
        let root = PathBuf::from(std::env::var_os("HOME").unwrap())
            .join("Desktop/test/antiquota-ask")
            .join(format!("{}", std::process::id()));
        let home = root.join("home");
        let auth = home.join(".gemini/antigravity-cli/antigravity-oauth-token");
        fs::create_dir_all(auth.parent().unwrap()).unwrap();
        fs::write(&auth, "synthetic").unwrap();
        let request = || AskRequest {
            prompt: "Listen to the audio file /x.webm.".into(),
            model: "gemini-3.8-flash-high".into(),
            cwd: root.to_string_lossy().into_owned(),
            slot: Slot {
                name: "one".into(),
                home: home.to_string_lossy().into_owned(),
            },
            add_dirs: vec!["/recordings".into()],
            timeout: 10.0,
        };
        let good = stub(&root, r#"{"status":"SUCCESS","response":"Words."}"#)
            .ask_agy(request(), Cancellation::default())
            .await
            .unwrap();
        assert_eq!(
            (good.status.as_str(), good.response.as_str()),
            ("SUCCESS", "Words.")
        );
        let argv = fs::read_to_string(root.join("argv")).unwrap();
        let argv: Vec<&str> = argv.lines().collect();
        assert!(argv.windows(2).any(|w| w == ["--add-dir", "/recordings"]));
        assert!(!argv.contains(&"--dangerously-skip-permissions"));
        let bad = stub(&root, "agy: something broke")
            .ask_agy(request(), Cancellation::default())
            .await
            .unwrap();
        assert_eq!(bad.status, "error");
        assert!(bad.error.contains("something broke"));
        let _ = fs::remove_dir_all(root);
    }
}
