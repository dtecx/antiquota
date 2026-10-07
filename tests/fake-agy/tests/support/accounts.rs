//! Proves scripted quota, identity, login, and per-account protocol behavior.
use super::{Fixture, frames};
use serde_json::{Value, json};
use std::{fs, io::Write, process::Stdio};

fn script(fixture: &Fixture, data: Value) -> std::path::PathBuf {
    let path = fixture.root.join("metadata.json");
    fs::write(&path, data.to_string()).unwrap();
    path
}
fn quota(fixture: &Fixture, script: &std::path::Path) -> std::process::Output {
    fixture
        .cmd("success")
        .env("FAKE_AGY_SCRIPT", script)
        .args(["-p", "/quota", "--output-format", "json"])
        .output()
        .unwrap()
}

#[test]
fn metadata_uses_slot_specific_identity_names_and_models_without_turns() {
    let fixture = Fixture::new();
    let script = script(
        &fixture,
        json!({"metadata_by_slot":{"one@example.com":{
            "identity":"different@example.com", "skills":["example"], "hooks":["antiquota"],
            "models":[{"id":"gemini-3.8-flash-low","label":"Flash Low"}], "config":{"mode":"default"}
        }}}),
    );
    let log = fixture.root.join("identity.log");
    let output = fixture
        .cmd("success")
        .env("FAKE_AGY_SCRIPT", &script)
        .args([
            "-p",
            "/quota",
            "--output-format",
            "json",
            "--log-file",
            log.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        fs::read_to_string(log)
            .unwrap()
            .contains("authenticated successfully as different@example.com")
    );
    for (command, expected) in [
        ("/skills", json!({"skills":[{"name":"example"}]})),
        ("/hooks", json!({"hooks":[{"name":"antiquota"}]})),
        ("/config", json!({"config":{"mode":"default"}})),
    ] {
        let output = fixture
            .cmd("success")
            .env("FAKE_AGY_SCRIPT", &script)
            .args(["-p", command, "--output-format", "json"])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(
            serde_json::from_slice::<Value>(&output.stdout).unwrap()["command"]["data"],
            expected
        );
    }
    let output = fixture
        .cmd("success")
        .env("FAKE_AGY_SCRIPT", script)
        .arg("models")
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "gemini-3.8-flash-low\tFlash Low\n"
    );
    assert!(
        !fixture
            .project
            .join(".fake-agy-test-session.turns")
            .exists()
    );
    assert!(!fixture.project.join("notes.txt").exists());
}
#[test]
fn quota_can_change_after_a_turn_and_preserve_disabled_window_shape() {
    let fixture = Fixture::new();
    let spent = json!({"status":"SUCCESS","command":{"name":"quota","data":{"groups":[
        {"name":"Gemini","buckets":[{"window":"5h","disabled":true},{"window":"weekly","remaining_fraction":0,"reset_time":"2099-01-07T00:00:00Z"}]},
        {"name":"Claude/GPT","buckets":[{"window":"5h","remaining_fraction":0.8,"reset_time":"2099-01-01T05:00:00Z"}]}
    ]}}});
    let script = script(&fixture, json!({"metadata":{"quota_after_turn":spent}}));
    assert_eq!(
        serde_json::from_slice::<Value>(&quota(&fixture, &script).stdout).unwrap()["command"]["data"]
            ["groups"][0]["buckets"][0]["remaining_fraction"],
        0.9
    );
    assert!(fixture.turn("success").status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&quota(&fixture, &script).stdout).unwrap(),
        spent
    );
}
#[test]
fn quota_errors_and_verification_are_real_error_channels() {
    let fixture = Fixture::new();
    for metadata in [
        json!({"quota_error":"UNAUTHENTICATED: token expired"}),
        json!({"verify_url":"https://accounts.google.com/signin/continue?fixture=1"}),
    ] {
        let script = script(&fixture, json!({"metadata":metadata}));
        let output = quota(&fixture, &script);
        assert!(!output.status.success());
        assert_eq!(
            serde_json::from_slice::<Value>(&output.stdout).unwrap()["status"],
            "ERROR"
        );
    }
    assert!(!fixture.project.join("notes.txt").exists());
}
#[test]
fn login_rejects_bad_codes_then_adopts_only_private_synthetic_credentials() {
    let fixture = Fixture::new();
    let auth = fixture
        .slot
        .join(".gemini/antigravity-cli/antigravity-oauth-token");
    fs::remove_file(&auth).unwrap();
    let script = script(&fixture, json!({"login":{"email":"three@example.com"}}));
    for code in ["wrong-code", "good-code"] {
        let log = fixture.root.join("login.log");
        let mut child = fixture
            .cmd("success")
            .env("FAKE_AGY_SCRIPT", &script)
            .args([
                "-p",
                "/quota",
                "--output-format",
                "json",
                "--log-file",
                log.to_str().unwrap(),
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(format!("{code}\n").as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(String::from_utf8_lossy(&output.stderr).contains("client_id=stub&state=stub"));
        if code == "wrong-code" {
            assert!(!output.status.success());
            assert!(!auth.exists());
            assert!(!log.exists());
        } else {
            assert!(output.status.success());
            assert_eq!(
                serde_json::from_slice::<Value>(&output.stdout).unwrap()["status"],
                "SUCCESS"
            );
            assert!(
                fs::read_to_string(log)
                    .unwrap()
                    .contains("three@example.com")
            );
            assert!(
                serde_json::from_slice::<Value>(&fs::read(&auth).unwrap())
                    .unwrap()
                    .get("refresh_token")
                    .is_some()
            );
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                assert_eq!(
                    fs::metadata(&auth).unwrap().permissions().mode() & 0o777,
                    0o600
                );
            }
        }
    }
    assert!(!fixture.project.join("notes.txt").exists());
}
#[test]
fn slot_and_turn_scenarios_override_global_defaults_and_can_omit_init() {
    let fixture = Fixture::new();
    let script = script(
        &fixture,
        json!({"scenario":"success", "scenarios_by_slot_and_turn":{"one@example.com":["transient","conversation_lost","quota_no_init"]}}),
    );
    for (index, error) in ["UNAVAILABLE", "conversation", "RESOURCE_EXHAUSTED"]
        .into_iter()
        .enumerate()
    {
        let output = fixture
            .cmd("success")
            .env("FAKE_AGY_SCRIPT", &script)
            .args(["-p", "fixture", "--output-format", "stream-json"])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        let frames = frames(&output);
        assert!(
            frames.last().unwrap()["result"]["error"]
                .as_str()
                .unwrap()
                .contains(error)
        );
        assert_eq!(frames.iter().any(|e| e["event"] == "init"), index != 2);
    }
}

#[test]
fn login_can_complete_automatically_without_code_input() {
    let fixture = Fixture::new();
    let auth = fixture
        .slot
        .join(".gemini/antigravity-cli/antigravity-oauth-token");
    fs::remove_file(&auth).unwrap();
    let script = script(
        &fixture,
        json!({"login":{"email":"auto@example.com","auto_complete_ms":20}}),
    );
    let output = fixture
        .cmd("success")
        .env("FAKE_AGY_SCRIPT", script)
        .args(["-p", "/quota", "--output-format", "json"])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(auth.is_file());
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["status"],
        "SUCCESS"
    );
    assert!(!fixture.project.join("notes.txt").exists());
}
