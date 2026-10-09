//! Prove fake protocol scenarios and isolation without invoking real agy.
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};
static NEXT: AtomicU64 = AtomicU64::new(0);
#[path = "support/accounts.rs"]
mod accounts;
struct Fixture {
    root: PathBuf,
    home: PathBuf,
    data: PathBuf,
    slot: PathBuf,
    project: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let owner = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .expect("user home");
        let root = PathBuf::from(owner)
            .join("Desktop/test/antiquota-fake-agy-tests")
            .join(format!(
                "{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        let home = root.join("home");
        let data = root.join("data");
        let slot = data.join("slots/one@example.com");
        let project = home.join("project");
        for p in [&home, &data, &slot, &project] {
            fs::create_dir_all(p).unwrap();
        }
        let auth = slot.join(".gemini/antigravity-cli/antigravity-oauth-token");
        fs::create_dir_all(auth.parent().unwrap()).unwrap();
        fs::write(&auth, "synthetic-token").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(auth, fs::Permissions::from_mode(0o600)).unwrap();
        }
        Self {
            root,
            home,
            data,
            slot,
            project,
        }
    }
    fn cmd(&self, scenario: &str) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_fake-agy"));
        command
            .current_dir(&self.project)
            .env("ANTIQUOTA_HOME", &self.home)
            .env("ANTIQUOTA_DIR", &self.data)
            .env("ANTIQUOTA_NO_USAGE_LINK", "1")
            .env("HOME", &self.slot)
            .env("USERPROFILE", &self.slot)
            .env("SSH_CONNECTION", "127.0.0.1 0 127.0.0.1 0")
            .env("ANTIQUOTA_SLOT", "one@example.com")
            .env("ANTIQUOTA_SESSION", "test-session")
            .env("FAKE_AGY_RECORD", self.root.join("calls.jsonl"))
            .env("FAKE_AGY_SCENARIO", scenario)
            .env_remove("FAKE_AGY_SCRIPT")
            .env_remove("STUB_SLEEP");
        command
    }
    fn turn(&self, scenario: &str) -> std::process::Output {
        self.cmd(scenario)
            .args([
                "-p",
                "hello",
                "--output-format",
                "stream-json",
                "--model",
                "gemini-3.8-flash-high",
                "--conversation",
                "resumed-123",
            ])
            .output()
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn frames(output: &std::process::Output) -> Vec<Value> {
    String::from_utf8(output.stdout.clone())
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[test]
fn process_plays_every_scenario_and_preserves_current_turn_usage() {
    for scenario in [
        "success",
        "quota",
        "rate",
        "auth",
        "approval",
        "hook",
        "background",
        "goal_complete",
        "huge",
        "crash",
        "stale_result",
        "duplicate_done",
        "transient",
        "conversation_lost",
    ] {
        let fixture = Fixture::new();
        let output = fixture.turn(scenario);
        let events = frames(&output);
        assert_eq!(events[0]["event"], "init");
        assert_eq!(events[0]["conversation_id"], "resumed-123");
        assert_eq!(
            output.status.code().unwrap(),
            if scenario == "crash" {
                23
            } else if ["quota", "rate", "auth", "transient", "conversation_lost"]
                .contains(&scenario)
            {
                1
            } else {
                0
            }
        );
        if scenario == "huge" {
            assert!(output.stdout.len() > 2_000_000);
        }
        if scenario == "background" {
            assert!(
                events.last().unwrap()["result"]["response"]
                    .as_str()
                    .unwrap()
                    .contains("RUN_IN_BACKGROUND:")
            );
        }
        if scenario == "duplicate_done" {
            assert_eq!(
                events
                    .iter()
                    .filter(|e| e["step_update"]["state"] == "DONE"
                        && e["step_update"]["step_type"] == "agent_response")
                    .count(),
                2
            );
        }
        if scenario == "stale_result" {
            assert_eq!(events.last().unwrap()["result"]["status"], "ERROR");
            assert!(
                events
                    .iter()
                    .any(|e| e["step_update"]["usage"]["input_tokens"] == 100)
            );
        }
        let recorded: Value = serde_json::from_str(
            fs::read_to_string(fixture.root.join("calls.jsonl"))
                .unwrap()
                .lines()
                .next()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(recorded["slot"], "one@example.com");
        assert_eq!(recorded["ssh_connection_present"], true);
        assert!(!recorded.to_string().contains("synthetic-token"));
    }
}
#[test]
fn quota_wall_arrives_mid_turn_after_a_tool_and_partial_text() {
    let fixture = Fixture::new();
    let output = fixture.turn("quota");
    assert_eq!(output.status.code(), Some(1));
    let steps: Vec<String> = frames(&output)
        .iter()
        .map(|frame| match frame["event"].as_str().unwrap_or("") {
            "step_update" => {
                let step = &frame["step_update"];
                format!(
                    "{} {}",
                    step["step_type"].as_str().unwrap_or(""),
                    step["state"].as_str().unwrap_or("")
                )
            }
            "result" => format!(
                "result {}",
                frame["result"]["status"].as_str().unwrap_or("")
            ),
            other => other.to_owned(),
        })
        .collect();
    let tool = steps
        .iter()
        .position(|s| s == "tool DONE")
        .expect("a finished tool");
    let text = steps
        .iter()
        .position(|s| s == "agent_response ACTIVE")
        .expect("partial reply text");
    assert_eq!(steps.last().map(String::as_str), Some("result ERROR"));
    assert!(
        steps[0] == "init" && tool < text && text + 1 == steps.len() - 1,
        "{steps:?}"
    );
    let all = frames(&output);
    assert_eq!(
        all[text]["step_update"]["text_delta"],
        "Partial answer before quota."
    );
    let error = all.last().unwrap()["result"]["error"].to_string();
    assert!(error.contains("RESOURCE_EXHAUSTED"), "{error}");
}
#[test]
fn delay_can_be_cancelled_before_any_edit() {
    let fixture = Fixture::new();
    let mut child = fixture
        .cmd("delay")
        .args(["-p", "long turn", "--output-format", "stream-json"])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    let mut reader = BufReader::new(child.stdout.take().unwrap());
    reader.read_line(&mut line).unwrap();
    assert!(line.contains("init"));
    let start = Instant::now();
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(start.elapsed().as_secs() < 2);
    assert!(!fixture.project.join("notes.txt").exists());
}
#[test]
fn metadata_commands_are_json_or_model_rows_and_never_edit_project() {
    let fixture = Fixture::new();
    for command in ["/quota", "/config", "/hooks", "/skills", "/models"] {
        let output = fixture
            .cmd("success")
            .args(["-p", command, "--output-format", "json"])
            .output()
            .unwrap();
        assert!(output.status.success());
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(value.get("command").is_some());
    }
    assert!(!fixture.project.join("notes.txt").exists());
    let models = fixture.cmd("success").arg("models").output().unwrap();
    assert!(
        String::from_utf8(models.stdout)
            .unwrap()
            .contains("gemini-3.8-flash-high\t")
    );
}
#[test]
fn custom_script_and_slot_overrides_are_reproducible() {
    let fixture = Fixture::new();
    let path = fixture.root.join("scenario.json");
    fs::write(
        &path,
        json!({"scenarios_by_slot":{"one@example.com":"auth"}}).to_string(),
    )
    .unwrap();
    let output = fixture
        .cmd("success")
        .env("FAKE_AGY_SCRIPT", &path)
        .args(["-p", "test", "--output-format", "stream-json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("UNAUTHENTICATED")
    );
}
#[test]
fn missing_isolation_refuses_before_recording_or_editing() {
    let fixture = Fixture::new();
    let output = fixture
        .cmd("success")
        .env_remove("ANTIQUOTA_HOME")
        .args(["-p", "hello"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(!fixture.root.join("calls.jsonl").exists());
    assert!(!fixture.project.join("notes.txt").exists());
}
#[cfg(unix)]
#[test]
fn symlinked_home_escape_refuses_before_recording() {
    let fixture = Fixture::new();
    let path = fixture.root.join("escape");
    std::os::unix::fs::symlink("/", &path).unwrap();
    let output = fixture
        .cmd("success")
        .env("ANTIQUOTA_HOME", path)
        .args(["-p", "hello"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(!fixture.root.join("calls.jsonl").exists());
}
#[cfg(unix)]
#[test]
fn hook_invocation_obeys_denial_without_executing_a_tool_command() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    let hook = fixture.root.join("fake-hook");
    fs::write(&hook, "#!/bin/sh\nprintf '%s' '{\"decision\":\"deny\"}'\n").unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o700)).unwrap();
    let script = fixture.root.join("hook.json");
    fs::write(
        &script,
        json!({"scenario":"hook","hook_command":[hook]}).to_string(),
    )
    .unwrap();
    let output = fixture
        .cmd("hook")
        .env("FAKE_AGY_SCRIPT", &script)
        .args(["-p", "hook fixture", "--output-format", "stream-json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!fixture.project.join("fake-hook-marker").exists());
    assert!(!fixture.project.join("notes.txt").exists());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("Tool refused")
    );
}
