//! Owns fail-closed validation of token context and the installed denying hook.
use crate::AgyRunner;
use antiquota_core::{PortError, TurnRequest};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Stdio,
};
use tokio::{io::AsyncWriteExt, process::Command};
pub fn isolated_fake(executable: &Path, root: &Path) -> bool {
    let file = executable
        .file_name()
        .and_then(|x| x.to_str())
        .unwrap_or("");
    let home =
        std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
    let Some(home) = home else { return false };
    let Ok(root) = root.canonicalize() else {
        return false;
    };
    let Ok(test) = home.join("Desktop/test").canonicalize() else {
        return false;
    };
    matches!(file, "fake-agy" | "fake-agy.exe" | "stub_agy.py")
        && root.starts_with(test)
        && std::env::var_os("ANTIQUOTA_HOME").is_some()
        && std::env::var_os("ANTIQUOTA_DIR").is_some()
        && std::env::var("ANTIQUOTA_NO_USAGE_LINK").as_deref() == Ok("1")
}
impl AgyRunner {
    pub(crate) async fn preflight(&self, request: &TurnRequest) -> Result<(), PortError> {
        let auth =
            Path::new(&request.slot.home).join(".gemini/antigravity-cli/antigravity-oauth-token");
        if !auth.is_file() || auth.is_symlink() {
            return Err(PortError::Invalid(
                "account token must be a regular private file".into(),
            ));
        }
        if self.isolated_fake {
            return Ok(());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if fs::metadata(&auth)
                .map_err(|_| PortError::Invalid("account token is unreadable".into()))?
                .permissions()
                .mode()
                & 0o077
                != 0
            {
                return Err(PortError::Invalid("account token must be private".into()));
            }
        }
        let token: serde_json::Value = serde_json::from_slice(
            &fs::read(&auth)
                .map_err(|_| PortError::Invalid("account token is unreadable".into()))?,
        )
        .map_err(|_| PortError::Invalid("account token is malformed".into()))?;
        if token["token"]["access_token"].as_str().is_none() {
            return Err(PortError::Invalid("account token is malformed".into()));
        }
        let root = Path::new(&request.state_dir)
            .parent()
            .ok_or_else(|| PortError::Invalid("invalid engine state directory".into()))?;
        let hooks: serde_json::Value = fs::read(root.join("stack/.gemini/config/hooks.json"))
            .ok()
            .and_then(|x| serde_json::from_slice(&x).ok())
            .ok_or_else(|| PortError::Invalid("owned denying hook is missing".into()))?;
        let hook = &hooks["antiquota"];
        let expected = format!("\"{}\" hook", self.hook_program.display());
        let valid = hook["enabled"] == true
            && hook["PreToolUse"].as_array().is_some_and(|rows| {
                rows.iter().any(|row| {
                    row["matcher"] == "*"
                        && row["hooks"].as_array().is_some_and(|items| {
                            items.iter().any(|item| item["command"] == expected)
                        })
                })
            });
        if !valid {
            return Err(PortError::Invalid(
                "owned denying hook is disabled or unusable".into(),
            ));
        }
        let account_hooks = Path::new(&request.slot.home).join(".gemini/config/hooks.json");
        let expected_hooks = root.join("stack/.gemini/config/hooks.json");
        if account_hooks.canonicalize().ok() != expected_hooks.canonicalize().ok() {
            return Err(PortError::Invalid(
                "owned denying hook is not installed for the account".into(),
            ));
        }
        let mut child = Command::new(&self.hook_program)
            .arg("hook")
            .env("ANTIQUOTA_STATE", &request.state_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| PortError::Invalid("owned denying hook cannot start".into()))?;
        let mut input = child
            .stdin
            .take()
            .ok_or_else(|| PortError::Invalid("hook stdin unavailable".into()))?;
        input
            .write_all(
                br#"{"tool_name":"run_command","tool_input":{"CommandLine":"sh -c whoami"}}"#,
            )
            .await
            .map_err(|_| PortError::Invalid("hook preflight failed".into()))?;
        drop(input);
        let output =
            tokio::time::timeout(std::time::Duration::from_secs(2), child.wait_with_output())
                .await
                .map_err(|_| PortError::Invalid("hook preflight timed out".into()))?
                .map_err(|_| PortError::Invalid("hook preflight failed".into()))?;
        let verdict: serde_json::Value = serde_json::from_slice(&output.stdout)
            .map_err(|_| PortError::Invalid("hook preflight returned no verdict".into()))?;
        if verdict["decision"] != "deny" {
            return Err(PortError::Invalid(
                "owned hook failed the denial preflight".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use antiquota_core::{Mode, Slot};
    #[tokio::test]
    async fn real_launch_refuses_missing_malformed_and_disabled_protection() {
        let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).unwrap();
        let root = PathBuf::from(home)
            .join("Desktop/test/antiquota-preflight")
            .join(format!(
                "{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        let slot = root.join("slots/one");
        let auth = slot.join(".gemini/antigravity-cli/antigravity-oauth-token");
        fs::create_dir_all(auth.parent().unwrap()).unwrap();
        let request = TurnRequest {
            session: "abcdef123456".into(),
            turn: 1,
            prompt: "Never launched".into(),
            model: "gemini-3.8-flash-high".into(),
            cwd: root.to_string_lossy().into_owned(),
            mode: Mode::Default,
            conversation_id: None,
            slot: Slot {
                name: "one".into(),
                home: slot.to_string_lossy().into_owned(),
            },
            state_dir: root.join("state").to_string_lossy().into_owned(),
        };
        let runner = AgyRunner {
            executable: "not-executed".into(),
            hook_program: "not-executed".into(),
            isolated_fake: false,
        };
        assert!(
            runner
                .preflight(&request)
                .await
                .unwrap_err()
                .to_string()
                .contains("regular private file")
        );
        fs::write(&auth, "malformed").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&auth, fs::Permissions::from_mode(0o600)).unwrap();
        }
        assert!(
            runner
                .preflight(&request)
                .await
                .unwrap_err()
                .to_string()
                .contains("malformed")
        );
        fs::write(&auth, r#"{"token":{"access_token":"synthetic"}}"#).unwrap();
        assert!(
            runner
                .preflight(&request)
                .await
                .unwrap_err()
                .to_string()
                .contains("missing")
        );
        let hooks = root.join("stack/.gemini/config/hooks.json");
        fs::create_dir_all(hooks.parent().unwrap()).unwrap();
        fs::write(&hooks, r#"{"antiquota":{"enabled":false,"PreToolUse":[]}}"#).unwrap();
        assert!(
            runner
                .preflight(&request)
                .await
                .unwrap_err()
                .to_string()
                .contains("disabled")
        );
        assert_eq!(
            fs::read_to_string(&auth).unwrap(),
            r#"{"token":{"access_token":"synthetic"}}"#
        );
        fs::remove_dir_all(root).unwrap();
    }
}
