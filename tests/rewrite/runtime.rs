//! Proves failed CLI admission and browser-launch flags using isolated fixtures.
use crate::support::{Server, bounded_output};
use serde_json::json;
use std::fs;

#[test]
fn raw_cli_without_accounts_exits_without_creating_a_session_or_launching_agy() {
    let mut server = Server::new_stopped("no-accounts", false);
    fs::remove_dir_all(server.data.join("slots")).unwrap();
    server.start();
    let output = bounded_output(
        server
            .command()
            .args(["run", "--", "-p", "No account fixture."]),
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("no signed-in account"));
    let state = server.call("GET", "/api/state", None).json();
    assert_eq!(state["accounts"], json!([]));
    assert_eq!(state["sessions"], json!([]));
    assert!(!server.root.join("fake-calls.jsonl").exists());
    server.evidence(
        "no-accounts",
        &json!({"cliFailedPromptly":true,"sessionsCreated":0,"runnerCalls":0}),
    );
    server.stop();
}

#[test]
fn http_without_accounts_preserves_accepted_prompt_and_returns_idle_without_runner() {
    let mut server = Server::new_stopped("http-no-accounts", false);
    fs::remove_dir_all(server.data.join("slots")).unwrap();
    server.start();
    let id = server.create();
    let sent = server.call(
        "POST",
        &format!("/api/sessions/{id}/send"),
        Some(json!({"text":"Accepted HTTP prompt without accounts."})),
    );
    assert_eq!(sent.status, 200);
    assert_eq!(sent.json()["result"], "started");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    let completed = loop {
        let response = server
            .call("GET", &format!("/api/sessions/{id}"), None)
            .json();
        if response["meta"]["status"] == "idle"
            && response["events"]
                .as_array()
                .unwrap()
                .iter()
                .any(|event| event["kind"] == "noaccount")
        {
            break response;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "no-account response incomplete: {response}"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    assert!(
        completed["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["t"] == "user"
                && event["text"] == "Accepted HTTP prompt without accounts.")
    );
    assert_eq!(completed["meta"]["turns"], 0);
    assert!(!server.root.join("fake-calls.jsonl").exists());
    server.evidence("no-accounts-http",&json!({"acceptedPromptDurable":true,"noaccountNotice":true,"sessionIdle":true,"runnerCalls":0}));
    server.stop();
}

#[cfg(unix)]
#[test]
fn serve_defaults_to_no_browser_and_gui_respects_no_open() {
    use std::{
        os::unix::fs::PermissionsExt,
        thread,
        time::{Duration, Instant},
    };
    let mut server = Server::new("browser-flags");
    let bin = server.root.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let launcher = bin.join(if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    });
    fs::write(
        &launcher,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" >> \"$ANTIQUOTA_BROWSER_RECORD\"\n",
    )
    .unwrap();
    fs::set_permissions(launcher, fs::Permissions::from_mode(0o700)).unwrap();
    let existing = std::env::var_os("PATH").unwrap();
    let paths = std::iter::once(bin).chain(std::env::split_paths(&existing));
    let path = std::env::join_paths(paths).unwrap();
    let record = server.root.join("browser-record.txt");
    for args in [
        ["serve", "--port", "0"].as_slice(),
        ["gui", "--port", "0", "--no-open"].as_slice(),
    ] {
        let output = bounded_output(
            server
                .command()
                .env("PATH", &path)
                .env("ANTIQUOTA_BROWSER_RECORD", &record)
                .args(args),
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert!(
        !record.exists(),
        "serve and gui --no-open must not launch a browser"
    );
    let output = bounded_output(
        server
            .command()
            .env("PATH", &path)
            .env("ANTIQUOTA_BROWSER_RECORD", &record)
            .args(["gui", "--port", "0"]),
    );
    assert!(output.status.success());
    let deadline = Instant::now() + Duration::from_secs(2);
    while !record.exists() {
        assert!(Instant::now() < deadline, "gui opener not invoked");
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        fs::read_to_string(record).unwrap(),
        format!("http://127.0.0.1:{}/\n", server.port)
    );
    server.evidence("browser-flags", &json!({"serveOpenedBrowser":false,"guiNoOpenOpenedBrowser":false,"guiInvokedFixtureOpener":true}));
    server.stop();
}
