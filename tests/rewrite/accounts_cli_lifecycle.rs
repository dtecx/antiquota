//! Proves raw CLI result selection, account-only persistence, and pipe cleanup.
use crate::{
    setup,
    support::{Server, bounded_output},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    io::{BufRead, BufReader},
    process::Stdio,
    thread,
    time::{Duration, Instant},
};

#[test]
fn raw_current_success_ignores_historical_quota_error_without_walling_account() {
    let server = Server::new_stopped("raw-stale-result", true);
    setup::script(&server, json!({"scenario":"stale_result"}));
    let quota_before = fs::read(server.data.join("state/quota/one@example.com.json")).unwrap();
    let args = [
        "run",
        "--slot",
        setup::ONE,
        "--",
        "-p",
        "Current response succeeds.",
        "--output-format",
        "stream-json",
    ];
    let output = bounded_output(server.command().args(args));
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    let frames: Vec<Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert!(
        frames
            .iter()
            .any(|row| row["step_update"]["step_type"] == "agent_response"
                && row["step_update"]["state"] == "DONE")
    );
    assert_eq!(frames.last().unwrap()["result"]["status"], "ERROR");
    assert!(
        text.contains("Individual quota reached"),
        "raw output must remain unchanged"
    );
    assert_eq!(setup::turn_calls(&server)[0]["argv"], json!(args[4..]));
    assert_eq!(
        fs::read(server.data.join("state/quota/one@example.com.json")).unwrap(),
        quota_before
    );
    assert!(
        !server
            .data
            .join("state/walls/one@example.com.gemini")
            .exists()
    );
    let call: Value = serde_json::from_str(
        fs::read_to_string(server.data.join("state/calls.jsonl"))
            .unwrap()
            .lines()
            .last()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(call["walled"], false);
    assert!(!server.data.join("state/failures").exists());
    setup::evidence(
        "raw-current-result",
        json!({"exit":0,"historicalErrorPreservedInOutput":true,"currentDoneWins":true,"wallsCreated":0,"argvExact":true}),
    );
}

fn session_bytes(server: &Server, id: &str) -> BTreeMap<String, Option<Vec<u8>>> {
    [
        format!("sessions/{id}/meta.json"),
        format!("sessions/{id}/events.jsonl"),
        "state/usage.jsonl".into(),
        "state/engine.json".into(),
    ]
    .into_iter()
    .map(|path| {
        let value = fs::read(server.data.join(&path)).ok();
        (path, value)
    })
    .collect()
}

#[test]
fn account_cli_probe_and_raw_quota_do_not_reconcile_active_engine_sessions() {
    let mut server = Server::new_stopped("cli-active-session", true);
    setup::start(&mut server);
    setup::script(&server, json!({"delay_ms":10000}));
    let id = server.create();
    setup::send(&server, &id, "Keep this accepted session active.");
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let state = server
            .call("GET", &format!("/api/sessions/{id}"), None)
            .json();
        if state["meta"]["status"] == "running" && state["meta"]["conversation_id"].is_string() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "active turn not observed: {state}"
        );
        thread::sleep(Duration::from_millis(10));
    }
    let before = session_bytes(&server, &id);
    let probe = bounded_output(server.command().arg("probe"));
    assert!(
        probe.status.success(),
        "{}",
        String::from_utf8_lossy(&probe.stderr)
    );
    assert!(String::from_utf8_lossy(&probe.stdout).contains("one@example.com: busy, skipped"));
    assert_eq!(session_bytes(&server, &id), before);
    setup::script(
        &server,
        json!({"scenarios_by_slot":{setup::TWO:"quota"},"metadata_by_slot":{setup::TWO:{"quota_after_turn":setup::healthy_quota(0.01)}}}),
    );
    let output = bounded_output(server.command().args([
        "run",
        "--slot",
        setup::TWO,
        "--",
        "-p",
        "Account-only quota path.",
        "--output-format",
        "stream-json",
    ]));
    assert_eq!(output.status.code(), Some(75));
    assert_eq!(session_bytes(&server, &id), before);
    assert!(
        server
            .data
            .join("state/walls/two@example.com.gemini")
            .is_file()
    );
    assert_eq!(
        server
            .call("POST", &format!("/api/sessions/{id}/stop"), Some(json!({})))
            .status,
        200
    );
    server.idle(&id);
    setup::evidence(
        "cli-active-session",
        json!({"sessionFilesUnchanged":true,"usageBytesUnchanged":true,"engineDiscoveryUnchanged":true,"busyAccountSkipped":true,"accountQuotaWallCreated":true}),
    );
    server.stop();
}

fn assert_no_delayed_marker(marker: &std::path::Path) {
    let until = Instant::now() + Duration::from_millis(650);
    while Instant::now() < until {
        assert!(!marker.exists(), "raw descendant survived");
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn raw_completed_parent_kills_descendants_and_closes_inherited_output_pipes() {
    let server = Server::new_stopped("raw-inherited-pipes", true);
    let marker = server.project.join("raw-descendant-marker");
    setup::script(
        &server,
        json!({"output_inherited_pipe_ms":500,"output_descendant_marker":marker}),
    );
    let started = Instant::now();
    let output = bounded_output(server.command().args([
        "run",
        "--slot",
        setup::ONE,
        "--",
        "-p",
        "Raw inherited pipes.",
        "--output-format",
        "stream-json",
    ]));
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(started.elapsed() < Duration::from_secs(2));
    assert_no_delayed_marker(&marker);
    setup::evidence(
        "raw-inherited-pipes",
        json!({"boundedSeconds":2,"delayedWritePrevented":true,"exit":0}),
    );
}

#[test]
fn closed_raw_stdout_fails_promptly_and_kills_process_tree() {
    let server = Server::new_stopped("raw-closed-stdout", true);
    let marker = server.project.join("raw-broken-pipe-marker");
    setup::script(
        &server,
        json!({"scenario":"huge","huge_bytes":8_000_000,"output_inherited_pipe_ms":500,"output_descendant_marker":marker}),
    );
    let log = fs::File::create(server.root.join("raw-closed-stderr.log")).unwrap();
    let mut child = server
        .command()
        .args([
            "run",
            "--slot",
            setup::ONE,
            "--",
            "-p",
            "Close raw output.",
            "--output-format",
            "stream-json",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::from(log))
        .spawn()
        .unwrap();
    let mut reader = BufReader::new(child.stdout.take().unwrap());
    let mut first = String::new();
    reader.read_line(&mut first).unwrap();
    assert!(first.contains("init"));
    drop(reader);
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if started.elapsed() >= Duration::from_secs(2) {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("closed output blocked raw process");
        }
        thread::sleep(Duration::from_millis(10));
    };
    assert!(!status.success());
    assert_no_delayed_marker(&marker);
    setup::evidence(
        "raw-closed-output",
        json!({"boundedSeconds":2,"failedExit":true,"delayedWritePrevented":true}),
    );
}
