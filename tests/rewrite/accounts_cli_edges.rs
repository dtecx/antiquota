//! Proves CLI usage errors, account listing details, probe and unwall targets, raw-run edges
//! and fixed engine ports.
use crate::{
    setup,
    support::{Server, bounded_output},
};
use serde_json::{Value, json};
use std::{
    fs,
    net::TcpListener,
    process::Stdio,
    thread,
    time::{Duration, Instant},
};

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn unknown_commands_and_bad_forms_print_usage_and_exit_2() {
    let server = Server::new_stopped("cli-usage", false);
    for args in [
        &["chat"][..],
        &["stack", "wipe"],
        &["unwall"],
        &["unwall", "one@example.com", "two@example.com"],
        &["serve", "--port", "eighty"],
        &["accounts", "extra"],
    ] {
        let output = bounded_output(server.command().args(args));
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
        assert!(
            text(&output.stderr).contains("Usage: antiquota"),
            "{args:?}"
        );
    }
    // Nothing ran: no session, no account change and no agy call.
    assert!(!server.data.join("sessions").exists());
    assert!(!server.root.join("fake-calls.jsonl").exists());
}

#[test]
fn accounts_lists_quota_walls_and_busy_leases_and_hints_when_empty() {
    let empty = Server::new_stopped("cli-accounts-empty", false);
    fs::remove_dir_all(empty.data.join("slots")).unwrap();
    let output = bounded_output(&mut empty.command());
    assert!(output.status.success());
    assert_eq!(
        text(&output.stdout),
        "no signed-in account; run: antiquota login\n"
    );
    let server = Server::new_stopped("cli-accounts-table", true);
    setup::write(
        &server.data.join("state/quota/one@example.com.json"),
        &json!({"at": 1.0, "quota": {"gemini": {"5h": [0.42, 4070926800.0]}, "cg": {"weekly": [0.07, 4071427200.0]}}}).to_string(),
    );
    setup::write(
        &server.data.join("state/walls/two@example.com.gemini"),
        "4071427320 quota panel\n",
    );
    let lease = antiquota_store::lock_file(&server.data.join("state/leases/one@example.com.lock"))
        .unwrap()
        .expect("hold the first account's lease");
    let output = bounded_output(server.command().arg("accounts"));
    drop(lease);
    assert!(output.status.success());
    let table = text(&output.stdout);
    let row = |slot: &str| {
        table
            .lines()
            .find(|l| l.starts_with(slot))
            .unwrap()
            .to_owned()
    };
    let one = row(setup::ONE);
    assert!(one.contains("5h 42%") && one.contains("weekly 7%"), "{one}");
    assert!(one.trim_end().ends_with("busy"), "{one}");
    let two = row(setup::TWO);
    assert!(two.contains("walled to "), "{two}");
    assert!(
        two.contains("weekly 90%"),
        "the other bucket still shows quota: {two}"
    );
    assert!(two.trim_end().ends_with("free"), "{two}");
}

#[test]
fn probe_reports_an_unreadable_panel_without_walling_the_account() {
    let server = Server::new_stopped("cli-probe-unreadable", true);
    setup::script(
        &server,
        json!({"metadata_by_slot": {setup::TWO: {"quota": {"status": "SUCCESS", "command": {"name": "quota", "data": {}}}}}}),
    );
    let output = bounded_output(server.command().arg("probe"));
    assert!(output.status.success(), "{}", text(&output.stderr));
    assert_eq!(
        text(&output.stdout),
        format!("{}: ok\n{}: unreadable\n", setup::ONE, setup::TWO)
    );
    assert!(
        !server
            .data
            .join("state/walls/two@example.com.gemini")
            .exists()
    );
    assert!(!server.data.join("state/walls/two@example.com.cg").exists());
}

#[test]
fn unwall_clears_both_buckets_of_the_named_account_only() {
    let server = Server::new_stopped("cli-unwall-named", true);
    let wall = |slot: &str, bucket: &str| server.data.join(format!("state/walls/{slot}.{bucket}"));
    for (slot, bucket) in [
        (setup::ONE, "gemini"),
        (setup::ONE, "cg"),
        (setup::TWO, "gemini"),
    ] {
        setup::write(&wall(slot, bucket), "4071427320 quota panel\n");
    }
    let output = bounded_output(server.command().args(["unwall", setup::ONE]));
    assert!(output.status.success());
    assert_eq!(
        text(&output.stdout),
        format!("walls cleared: {}\n", setup::ONE)
    );
    assert!(!wall(setup::ONE, "gemini").exists() && !wall(setup::ONE, "cg").exists());
    assert!(
        wall(setup::TWO, "gemini").exists(),
        "another account keeps its wall"
    );
}

#[test]
fn run_rejects_unknown_slots_forwards_without_a_separator_and_audits_failures() {
    let server = Server::new_stopped("cli-run-edges", true);
    let unknown =
        bounded_output(
            server
                .command()
                .args(["run", "--slot", "three@example.com", "-p", "x"]),
        );
    assert_eq!(unknown.status.code(), Some(1));
    assert!(text(&unknown.stderr).contains("no signed-in slot 'three@example.com'"));
    assert!(
        !server.root.join("fake-calls.jsonl").exists(),
        "nothing was launched"
    );
    // Without `--`, every argument after the optional slot still reaches agy unchanged.
    let args = [
        "run",
        "-p",
        "No separator.",
        "--output-format",
        "stream-json",
        "--model=gemini-3.8-flash-high",
    ];
    let output = bounded_output(server.command().args(args));
    assert!(output.status.success(), "{}", text(&output.stderr));
    let calls = setup::turn_calls(&server);
    assert_eq!(calls.last().unwrap()["argv"], json!(args[1..]));
    // A provider failure that is not quota returns agy's own exit code and keeps an audit.
    setup::script(&server, json!({"scenario": "crash"}));
    let crashed = bounded_output(server.command().args([
        "run",
        "--slot",
        setup::TWO,
        "-p",
        "Crash.",
        "--output-format",
        "stream-json",
    ]));
    assert_eq!(crashed.status.code(), Some(23));
    let failures: Vec<_> = fs::read_dir(server.data.join("state/failures"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(failures.len(), 1);
    let name = failures[0]
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    assert!(name.ends_with(&format!("-{}.log", setup::TWO)), "{name}");
    let log = fs::read_to_string(&failures[0]).unwrap();
    assert!(
        log.starts_with("rc=23\n--- stdout\n") && log.contains("--- stderr\nfake agy crashed"),
        "{log}"
    );
    let calls: Vec<Value> = fs::read_to_string(server.data.join("state/calls.jsonl"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let last = calls.last().unwrap();
    assert_eq!(
        (
            last["slot"].as_str(),
            last["rc"].as_i64(),
            last["walled"].as_bool()
        ),
        (Some(setup::TWO), Some(23), Some(false))
    );
    assert!(
        !server
            .data
            .join("state/walls/two@example.com.gemini")
            .exists()
    );
}

/// Starts `serve` or `gui` with the given arguments and returns its discovery record.
fn engine_on(server: &Server, args: &[&str]) -> (std::process::Child, Value) {
    let mut child = server
        .command()
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(12);
    loop {
        if let Ok(text) = fs::read_to_string(server.data.join("state/engine.json"))
            && let Ok(info) = serde_json::from_str::<Value>(&text)
            && info["pid"].as_u64() == Some(child.id().into())
        {
            return (child, info);
        }
        assert!(child.try_wait().unwrap().is_none(), "the engine exited");
        assert!(Instant::now() < deadline, "no discovery");
        thread::sleep(Duration::from_millis(20));
    }
}
fn shut_down(server: &Server, mut child: std::process::Child, port: u16) {
    let token = fs::read_to_string(server.data.join("state/gui-token")).unwrap();
    let headers = std::collections::BTreeMap::from([(
        "X-Antiquota-Token".to_owned(),
        token.trim().to_owned(),
    )]);
    let response = crate::support::http::request(port, "POST", "/api/shutdown", &headers, b"{}");
    assert_eq!(response.status, 200);
    let deadline = Instant::now() + Duration::from_secs(10);
    while child.try_wait().unwrap().is_none() {
        assert!(Instant::now() < deadline, "the engine did not stop");
        thread::sleep(Duration::from_millis(20));
    }
}
fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

#[test]
fn serve_and_gui_listen_on_a_requested_fixed_port() {
    let server = Server::new_stopped("cli-fixed-port", false);
    for command in ["serve", "gui"] {
        let port = free_port();
        let text_port = port.to_string();
        let (child, info) = engine_on(&server, &[command, "--port", &text_port, "--no-open"]);
        assert_eq!(info["port"], json!(port), "{command}");
        assert_eq!(
            info["url"],
            json!(format!("http://127.0.0.1:{port}/")),
            "{command}"
        );
        shut_down(&server, child, port);
    }
    assert!(
        !server.root.join("browser-record.txt").exists(),
        "--no-open opened nothing"
    );
}
