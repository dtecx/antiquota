//! Proves legacy account CLI commands and unmodified agy argument forwarding.
use crate::{
    setup,
    support::{Server, bounded_output},
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs, thread,
    time::{Duration, Instant},
};

#[test]
fn default_accounts_probe_stack_and_unwall_use_only_owned_state() {
    let mut server = Server::new_stopped("account-cli", true);
    setup::captured_home(&server);
    setup::start(&mut server);
    let default = bounded_output(&mut server.command());
    assert!(default.status.success());
    let accounts = bounded_output(server.command().arg("accounts"));
    assert!(accounts.status.success());
    assert_eq!(default.stdout, accounts.stdout);
    let text = String::from_utf8(accounts.stdout).unwrap();
    assert!(text.contains("ACCOUNT"));
    assert!(text.contains(setup::ONE) && text.contains(setup::TWO));
    assert!(text.contains("GEMINI") && text.contains("CLAUDE/GPT") && text.contains("LEASE"));
    let probe = bounded_output(server.command().arg("probe"));
    assert!(
        probe.status.success(),
        "{}",
        String::from_utf8_lossy(&probe.stderr)
    );
    assert!(String::from_utf8_lossy(&probe.stdout).contains("ok"));
    let stack = bounded_output(server.command().arg("stack"));
    assert!(stack.status.success());
    assert_eq!(
        String::from_utf8(stack.stdout).unwrap().trim(),
        server.data.join("stack/.gemini").to_str().unwrap()
    );
    setup::write(
        &server.data.join("state/walls/one@example.com.gemini"),
        "4071427320 test wall\n",
    );
    let unwall = bounded_output(server.command().args(["unwall", "all"]));
    assert!(unwall.status.success());
    assert_eq!(
        String::from_utf8(unwall.stdout).unwrap(),
        "walls cleared: all\n"
    );
    assert!(
        !server
            .data
            .join("state/walls/one@example.com.gemini")
            .exists()
    );
    assert_eq!(
        server.call("GET", "/api/state", None).json()["sessions"],
        json!([])
    );
    setup::evidence(
        "account-cli",
        json!({"default":"accounts","explicitAccountsMatches":true,"probe":true,"stack":true,"unwallAll":true,"sessionsCreated":0}),
    );
    server.stop();
}

#[test]
fn raw_run_preserves_argv_pin_output_and_quota_exit_75() {
    let mut server = Server::new_stopped("raw-cli", true);
    setup::start(&mut server);
    let args = [
        "run",
        "--slot",
        setup::TWO,
        "--",
        "-p",
        "Raw argument fixture.",
        "--output-format",
        "stream-json",
        "--model=gemini-3.8-flash-low",
        "--include-directories",
        "one directory,two",
        "--fake-future-flag",
        "literal;$()`text",
    ];
    let output = bounded_output(server.command().args(args));
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let calls = setup::turn_calls(&server);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0]["slot"], setup::TWO);
    assert_eq!(calls[0]["argv"], json!(args[4..]));
    assert_eq!(calls[0]["stdin_is_terminal"], false);
    assert_eq!(calls[0]["stdout_is_terminal"], false);
    assert_eq!(calls[0]["ssh_connection_present"], true);
    let frames: Vec<serde_json::Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(frames.last().unwrap()["event"], "result");
    setup::script(
        &server,
        json!({"scenarios_by_slot":{setup::ONE:"quota"},"metadata_by_slot":{setup::ONE:{"quota_after_turn":setup::healthy_quota(0.01)}}}),
    );
    let output = bounded_output(server.command().args([
        "run",
        "--slot",
        setup::ONE,
        "--",
        "-p",
        "Quota raw fixture.",
        "--output-format",
        "stream-json",
    ]));
    assert_eq!(output.status.code(), Some(75));
    assert!(String::from_utf8_lossy(&output.stdout).contains("RESOURCE_EXHAUSTED"));
    assert!(
        fs::read_to_string(server.data.join("state/walls/one@example.com.gemini"))
            .unwrap()
            .contains("quota panel")
    );
    let all_walled = bounded_output(server.command().args([
        "run",
        "--slot",
        setup::ONE,
        "--",
        "-p",
        "Should not launch.",
    ]));
    assert_eq!(all_walled.status.code(), Some(75));
    assert_eq!(setup::turn_calls(&server).len(), 2);
    assert_eq!(
        server.call("GET", "/api/state", None).json()["sessions"],
        json!([])
    );
    setup::evidence(
        "raw-cli",
        json!({"argvExact":true,"literalShellCharactersPreserved":true,"pinnedAccount":setup::TWO,"quotaExit":75,"allWalledExit":75,"rawOutputPreserved":true,"sessionsCreated":0}),
    );
    server.stop();
}

#[test]
fn interactive_raw_run_preserves_native_tty_without_foreground_control() {
    let mut server = Server::new_stopped("raw-cli-tty", true);
    setup::start(&mut server);
    let environment: BTreeMap<String, String> = [
        ("ANTIQUOTA_HOME", server.home.to_string_lossy().into_owned()),
        ("ANTIQUOTA_DIR", server.data.to_string_lossy().into_owned()),
        ("ANTIQUOTA_AGY", server.agy.to_string_lossy().into_owned()),
        ("ANTIQUOTA_NO_USAGE_LINK", "1".into()),
        (
            "FAKE_AGY_SCRIPT",
            server.script.to_string_lossy().into_owned(),
        ),
        (
            "FAKE_AGY_RECORD",
            server
                .root
                .join("fake-calls.jsonl")
                .to_string_lossy()
                .into_owned(),
        ),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_string(), value))
    .collect();
    let args = vec![
        "run".into(),
        "--slot".into(),
        setup::ONE.into(),
        "--".into(),
        "--model".into(),
        "gemini-3.8-flash-high".into(),
    ];
    let console = antiquota_pty::Console::spawn(
        &crate::support::binary("antiquota"),
        &args,
        &server.project,
        &environment,
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while !console.finished().unwrap() {
        assert!(
            Instant::now() < deadline,
            "interactive fixture did not exit: {}",
            console.text()
        );
        thread::sleep(Duration::from_millis(10));
    }
    console.reap();
    let calls: Vec<_> = setup::calls(&server)
        .into_iter()
        .filter(|row| setup::argument(row, "--model") == Some("gemini-3.8-flash-high"))
        .collect();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0]["argv"],
        json!(["--model", "gemini-3.8-flash-high"])
    );
    for stream in [
        "stdin_is_terminal",
        "stdout_is_terminal",
        "stderr_is_terminal",
    ] {
        assert_eq!(calls[0][stream], true, "{stream}");
    }
    assert_eq!(calls[0]["slot"], setup::ONE);
    setup::evidence(
        "raw-cli-tty",
        json!({"allStreamsAreNativeTty":true,"argvExact":true,"foregroundUiControl":false,"platform":std::env::consts::OS}),
    );
    drop(console);
    server.stop();
}
