//! End-to-end slice-one contract, transport security, streaming, and recovery.
#[path = "../../../tests/rewrite/runtime.rs"]
mod runtime;
#[path = "../../../tests/rewrite/support/mod.rs"]
mod support;
use serde_json::json;
use std::{collections::BTreeMap, fs, thread, time::Duration};
use support::{Server, http, replay::Replay};

#[test]
fn captured_slice_one_requests_replay_with_exact_json_fields() {
    let mut server = Server::new_stopped("replay", true);
    let mut captured_lease = Some(
        antiquota_store::lock_file(&server.data.join("state/leases/one@example.com.lock"))
            .unwrap()
            .expect("hold captured probe lease before background discovery"),
    );
    server.start();
    let mut replay = Replay::new(&server);
    for name in [
        "health",
        "bad-host",
        "missing-token",
        "wrong-token",
        "events-wrong-token",
        "unknown-static",
        "vendor-traversal",
        "body-limit",
        "invalid-json",
        "session-create",
        "session-get-empty",
        "session-patch",
        "session-patch-invalid",
        "session-missing",
        "session-create-invalid",
        "send-empty",
    ] {
        replay.case(&server, name);
        if name == "wrong-token" {
            replay.case(&server, "state-initial");
            drop(captured_lease.take());
        }
    }
    replay.case(&server, "shutdown");
    replay.evidence(&server);
    server.wait_stopped();
}
#[test]
fn whole_server_streams_one_turn_and_replays_after_restart() {
    let mut server = Server::new("turn");
    let mut replay = Replay::new(&server);
    let created = replay.case(&server, "session-create").json();
    let id = created["id"].as_str().unwrap().to_string();
    let state = server.call("GET", "/api/state", None);
    assert_eq!(state.status, 200);
    let state = state.json();
    assert_eq!(state["sessions"].as_array().unwrap().len(), 1);
    assert_eq!(state["sessions"][0]["id"], id);
    assert_eq!(state["sessions"][0]["status"], "idle");
    assert_eq!(state["defaults"]["model"], "gemini-3.8-flash-high");
    assert_eq!(state["accounts"][0]["slot"], "one@example.com");
    let mut events = http::Events::open(server.port, &server.token);
    replay.case(&server, "send");
    let mut kinds = Vec::new();
    let mut samples = Vec::new();
    loop {
        let event = events.next();
        assert!(event.get("ts").is_some());
        if event["sid"] == id {
            kinds.push(event["t"].as_str().unwrap().to_string());
            if matches!(
                event["t"].as_str().unwrap(),
                "user" | "turn_start" | "tool" | "delta" | "assistant" | "turn_end"
            ) {
                samples.push(event.clone());
            }
            if event["t"] == "turn_end" {
                break;
            }
        }
    }
    let completed = server.idle(&id);
    replay.first_turn_events(&samples);
    assert_eq!(completed["meta"]["usage"]["input"], 100);
    assert_eq!(completed["meta"]["usage"]["output"], 10);
    for kind in [
        "user",
        "turn_start",
        "tool",
        "delta",
        "assistant",
        "turn_end",
    ] {
        assert!(kinds.iter().any(|v| v == kind), "missing {kind}: {kinds:?}");
    }
    let token = server.token.clone();
    server.stop();
    server.start();
    assert_eq!(server.token, token);
    let recovered = server
        .call("GET", &format!("/api/sessions/{id}"), None)
        .json();
    assert_eq!(completed, recovered);
    let calls = fs::read_to_string(server.root.join("fake-calls.jsonl")).unwrap();
    assert!(calls.contains("ssh_connection_present\":true"));
    assert!(calls.contains("one@example.com"));
    assert!(!calls.contains("synthetic-account-token"));
    server.evidence("turn-restart",&json!({"eventKinds":kinds,"usage":recovered["meta"]["usage"],"tokenReused":true,"sessionRecovered":true}));
    server.stop();
}
#[test]
fn queued_messages_stop_delete_and_crash_leave_recoverable_sessions() {
    let mut server = Server::new("control");
    let id = server.create();
    let mut replay = Replay::new(&server);
    replay.bind_setup("{{SESSION_1}}", &id);
    fs::write(
        &server.script,
        json!({"scenario":"delay","delay_ms":300}).to_string(),
    )
    .unwrap();
    assert_eq!(
        server
            .call(
                "POST",
                &format!("/api/sessions/{id}/send"),
                Some(json!({"text":"First."}))
            )
            .json()["result"],
        "started"
    );
    assert_eq!(
        server
            .call(
                "POST",
                &format!("/api/sessions/{id}/send"),
                Some(json!({"text":"Second."}))
            )
            .json()["result"],
        "queued"
    );
    thread::sleep(Duration::from_millis(50));
    let queued = server.idle_turns(&id, 2);
    assert_eq!(queued["meta"]["turns"], 2);
    server.scenario("delay");
    let mut events = http::Events::open(server.port, &server.token);
    server.call(
        "POST",
        &format!("/api/sessions/{id}/send"),
        Some(json!({"text":"Stop this."})),
    );
    while events.next()["t"] != "turn_start" {}
    replay.case(&server, "session-stop");
    let stopped = server.idle_turns(&id, 3);
    assert_eq!(stopped["meta"]["queue"], json!([]));
    server.scenario("crash");
    server.call(
        "POST",
        &format!("/api/sessions/{id}/send"),
        Some(json!({"text":"Crash fixture."})),
    );
    thread::sleep(Duration::from_millis(50));
    let crashed = server.idle_turns(&id, 4);
    assert!(
        crashed["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["t"] == "notice" && event["kind"] == "error")
    );
    assert_eq!(
        server
            .call("DELETE", &format!("/api/sessions/{id}"), None)
            .json(),
        json!({"ok":true})
    );
    assert_eq!(
        server
            .call("GET", &format!("/api/sessions/{id}"), None)
            .status,
        404
    );
    let second = server.create();
    replay.bind_setup("{{SESSION_2}}", &second);
    replay.case(&server, "session-delete");
    assert_eq!(
        server
            .call("GET", &format!("/api/sessions/{second}"), None)
            .status,
        404
    );
    server.evidence(
        "control",
        &json!({"queuedTurns":2,"stopClearedQueue":true,"crashRecovered":true,"deleted":true}),
    );
    server.stop();
}
#[test]
fn engine_discovery_reuses_lock_and_static_auth_remains_local() {
    let mut server = Server::new("discovery");
    let root = server.call("GET", "/", None);
    assert_eq!(root.status, 200);
    let text = String::from_utf8(root.body).unwrap();
    assert!(text.contains(&server.token));
    assert!(!text.contains("__ANTIQUOTA_TOKEN__"));
    let second =
        support::bounded_output(server.command().args(["serve", "--port", "0", "--no-open"]));
    assert!(second.status.success());
    assert!(
        String::from_utf8(second.stdout)
            .unwrap()
            .contains("already runs")
    );
    let health = http::request(server.port, "GET", "/api/health", &BTreeMap::new(), &[]);
    assert_eq!(health.status, 200);
    assert_eq!(
        http::request(
            server.port,
            "GET",
            "/api/health",
            &BTreeMap::from([("Host".into(), format!("localhost:{}", server.port))]),
            &[]
        )
        .status,
        200
    );
    assert!(
        std::net::TcpStream::connect_timeout(
            &std::net::SocketAddr::from(([127, 0, 0, 2], server.port)),
            Duration::from_millis(200)
        )
        .is_err()
    );
    assert_eq!(
        http::request(
            server.port,
            "GET",
            &format!("/api/state?token={}", server.token),
            &BTreeMap::new(),
            &[]
        )
        .status,
        403
    );
    assert_eq!(server.call("GET", "/api/events", None).status, 403);
    assert_eq!(server.call("GET", "/../Cargo.toml", None).status, 403);
    #[cfg(unix)]
    {
        let assets = server.root.join("ui/assets");
        fs::create_dir_all(&assets).unwrap();
        let private = server.project.join("private.js");
        fs::write(&private, "private fixture").unwrap();
        std::os::unix::fs::symlink(private, assets.join("escape.js")).unwrap();
        assert_eq!(server.call("GET", "/assets/escape.js", None).status, 403);
    }
    server.stop();
    assert!(!server.data.join("state/engine.json").exists());
    server.start();
    server.stop();
}

#[test]
fn resumed_stale_and_duplicate_results_preserve_usage_and_success() {
    let mut server = Server::new("resume");
    let id = server.create();
    let mut conversation = String::new();
    let mut evidence = Vec::new();
    for (index, scenario) in ["success", "stale_result", "duplicate_done", "huge"]
        .iter()
        .enumerate()
    {
        server.scenario(scenario);
        assert_eq!(
            server
                .call(
                    "POST",
                    &format!("/api/sessions/{id}/send"),
                    Some(json!({"text":"Short scripted turn."}))
                )
                .json()["result"],
            "started"
        );
        let completed = server.idle_turns(&id, index as u64 + 1);
        let current = completed["meta"]["conversation_id"].as_str().unwrap();
        if conversation.is_empty() {
            conversation = current.into();
        } else {
            assert_eq!(current, conversation);
        }
        assert_eq!(
            completed["meta"]["usage"]["input"],
            (index as u64 + 1) * 100
        );
        assert_eq!(
            completed["meta"]["usage"]["output"],
            (index as u64 + 1) * 10
        );
        let events = completed["events"].as_array().unwrap();
        let turn = index as u64 + 1;
        assert_eq!(
            events
                .iter()
                .filter(|e| e["t"] == "assistant" && e["turn"] == turn)
                .count(),
            1
        );
        assert_eq!(
            events.iter().rev().find(|e| e["t"] == "turn_end").unwrap()["status"],
            "SUCCESS"
        );
        assert!(
            !events
                .iter()
                .any(|e| e["t"] == "notice" && e["kind"] == "error")
        );
        if *scenario == "huge" {
            assert!(
                events
                    .iter()
                    .find(|e| e["t"] == "assistant" && e["turn"] == turn)
                    .unwrap()["text"]
                    .as_str()
                    .unwrap()
                    .len()
                    >= 2_000_000
            );
        }
        evidence.push(json!({"scenario":scenario,"turn":turn,"usage":completed["meta"]["usage"],"assistantReplies":1,"status":"SUCCESS"}));
    }
    server.evidence(
        "resume-scenarios",
        &json!({"sameConversation":true,"scenarios":evidence}),
    );
    server.stop();
}
