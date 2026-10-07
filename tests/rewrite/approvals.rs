//! Proves permission lists, approvals and mid-turn steering through the real engine and hook.
use crate::support::{Server, binary, replay::Replay};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

fn settings(server: &Server) -> std::path::PathBuf {
    server
        .data
        .join("stack/.gemini/antigravity-cli/settings.json")
}

fn seed_rules(server: &Server, permissions: Value) {
    let path = settings(server);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        path,
        json!({"permissions": permissions, "general": {"keep": true}}).to_string(),
    )
    .unwrap();
}

fn session(server: &Server, id: &str) -> Value {
    server
        .call("GET", &format!("/api/sessions/{id}"), None)
        .json()
}

fn wait_for(server: &Server, id: &str, what: &str, test: impl Fn(&Value) -> bool) -> Value {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let value = session(server, id);
        if test(&value) {
            return value;
        }
        assert!(Instant::now() < deadline, "{what}: {value}");
        thread::sleep(Duration::from_millis(20));
    }
}

fn approvals(value: &Value) -> Vec<Value> {
    value["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["t"] == "approval")
        .cloned()
        .collect()
}

#[test]
fn captured_permission_approval_and_steer_routes() {
    let mut server = Server::new_stopped("approvals-captured", false);
    seed_rules(&server, json!({"allow": ["command(ls)"]}));
    fs::write(&server.script, json!({"delay_ms": 1500}).to_string()).unwrap();
    server.start();
    let mut replay = Replay::new(&server);
    replay.case(&server, "permissions");
    replay.case(&server, "permissions-put");
    replay.case(&server, "permissions-error");
    let stored: Value = serde_json::from_slice(&fs::read(settings(&server)).unwrap()).unwrap();
    assert_eq!(stored["general"]["keep"], true, "unrelated settings stay");
    let id = server.create();
    replay.bind_setup("{{SESSION_1}}", &id);
    replay.case(&server, "send-long");
    replay.case(&server, "send-queued");
    replay.case(&server, "session-steer");
    let after = server.idle(&id);
    assert_eq!(
        after["meta"]["turns"], 1,
        "a steered message is not a new turn"
    );
    assert_eq!(after["meta"]["queue"], json!([]));
    let steered: Vec<&Value> = after["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["t"] == "user" && e["steered"] == true)
        .collect();
    assert_eq!(steered.len(), 1);
    assert_eq!(steered[0]["text"], "Steer this turn.");
    let empty = server.call(
        "POST",
        &format!("/api/sessions/{id}/steer"),
        Some(json!({})),
    );
    assert_eq!(empty.json(), json!({"messages": []}));
    for name in [
        "approval-request",
        "approval-wait",
        "approval-answer-invalid",
        "approval-answer",
        "approval-read-decided",
        "approval-request-allow",
        "approval-answer-allow",
        "approval-read-allowed",
        "approval-unknown",
        "approval-unknown-session",
        "approval-immediate-read",
    ] {
        replay.case(&server, name);
    }
    let value = session(&server, &id);
    let cards = approvals(&value);
    let states: Vec<&str> = cards.iter().map(|e| e["state"].as_str().unwrap()).collect();
    assert_eq!(states, ["pending", "denied", "pending", "allowed"]);
    assert_eq!(cards[1]["why"], "fixture denied");
    assert_eq!(cards[0]["args"]["CommandLine"], "touch approval-file");
    assert_eq!(cards[0]["rule"], "command(touch)");
    assert!(cards[0]["deadline"].as_f64().unwrap() > cards[0]["ts"].as_f64().unwrap() + 899.0);
    assert_eq!(value["meta"]["status"], "idle");
    let missing = server.call(
        "POST",
        "/api/approvals/missing/answer",
        Some(json!({"decision":"allow"})),
    );
    assert_eq!(missing.status, 404);
    server.stop();
}

/// Starts a turn whose fake agy invokes the real `antiquota hook` for one tool call.
fn hook_turn(server: &mut Server, command_line: &str) -> String {
    fs::write(
        &server.script,
        json!({
            "scenario": "approval",
            "hook_command": [binary("antiquota"), "hook", "pre-tool"],
            "hook_payload": {"toolCall": {"name": "run_command", "args": {"CommandLine": command_line}}},
        })
        .to_string(),
    )
    .unwrap();
    let id = server.create();
    let sent = server.call(
        "POST",
        &format!("/api/sessions/{id}/send"),
        Some(json!({"text": "Use the tool."})),
    );
    assert_eq!(sent.json(), json!({"result": "started"}));
    id
}

fn pending_id(server: &Server, id: &str) -> String {
    let value = wait_for(server, id, "approval card", |v| {
        v["meta"]["status"] == "approval" && approvals(v).iter().any(|e| e["state"] == "pending")
    });
    approvals(&value)[0]["id"].as_str().unwrap().into()
}

#[test]
fn hook_waits_for_the_user_and_relays_each_answer() {
    let mut server = Server::new_stopped("approvals-hook", false);
    seed_rules(&server, json!({"allow": ["command(ls)"]}));
    server.start();
    let hooks: Value = serde_json::from_slice(
        &fs::read(server.data.join("stack/.gemini/config/hooks.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(hooks["antiquota"]["enabled"], true);
    assert!(
        hooks["antiquota"]["PreToolUse"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .ends_with(" hook pre-tool")
    );

    // Allow once: the tool completes and no rule is stored.
    let id = hook_turn(&mut server, "touch allowed-once");
    let aid = pending_id(&server, &id);
    let answer = server.call(
        "POST",
        &format!("/api/approvals/{aid}/answer"),
        Some(json!({"decision": "allow", "always": false})),
    );
    assert_eq!(answer.json(), json!({"decision": "allow"}));
    let done = server.idle(&id);
    let tool = done["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["t"] == "tool")
        .unwrap();
    assert_eq!(tool["state"], "done", "{done}");
    let rules = server.call("GET", "/api/permissions", None).json();
    assert_eq!(rules, json!({"allow": ["command(ls)"]}));

    // Decline: agy hears the refusal and the tool fails.
    let id = hook_turn(&mut server, "touch declined");
    let aid = pending_id(&server, &id);
    server.call(
        "POST",
        &format!("/api/approvals/{aid}/answer"),
        Some(json!({"decision": "deny"})),
    );
    let done = server.idle(&id);
    let tool = done["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["t"] == "tool")
        .unwrap();
    assert_eq!(tool["state"], "error", "{done}");

    // Always allow stores the suggested rule, which then allows the next call at once.
    let id = hook_turn(&mut server, "touch always");
    let aid = pending_id(&server, &id);
    server.call(
        "POST",
        &format!("/api/approvals/{aid}/answer"),
        Some(json!({"decision": "allow", "always": true})),
    );
    server.idle(&id);
    let rules = server.call("GET", "/api/permissions", None).json();
    assert_eq!(rules, json!({"allow": ["command(ls)", "command(touch)"]}));
    let id = hook_turn(&mut server, "touch again");
    let done = server.idle(&id);
    assert!(approvals(&done).is_empty(), "a ruled command needs no card");
    server.stop();
}

#[test]
fn always_allow_never_stores_an_arbitrary_program_rule() {
    let mut server = Server::new_stopped("approvals-runner", false);
    seed_rules(&server, json!({"allow": []}));
    server.start();
    let id = server.create();
    let request = server.call(
        "POST",
        "/api/approvals",
        Some(json!({"session": id, "tool": "run_command", "args": {"CommandLine": "bash -c 'rm -rf x'"}})),
    );
    let aid = request.json()["pending"].as_str().unwrap().to_owned();
    let card = approvals(&session(&server, &id));
    assert_eq!(card[0]["rule"], "", "no permanent rule is offered for bash");
    server.call(
        "POST",
        &format!("/api/approvals/{aid}/answer"),
        Some(json!({"decision": "allow", "always": true})),
    );
    assert_eq!(
        server.call("GET", "/api/permissions", None).json(),
        json!({"allow": []})
    );
    let refused = server.call(
        "PUT",
        "/api/permissions",
        Some(json!({"allow": ["command(xargs)"]})),
    );
    assert_eq!(refused.status, 400);
    assert!(
        refused.json()["error"]
            .as_str()
            .unwrap()
            .contains("arbitrary programs")
    );
    // An unruled edit answered "always" lets this session make edits.
    let edit = server.call(
        "POST",
        "/api/approvals",
        Some(json!({"session": id, "tool": "write_to_file", "args": {"TargetFile": "a.txt", "CodeContent": "x".repeat(5000)}})),
    );
    let aid = edit.json()["pending"].as_str().unwrap().to_owned();
    let cards = approvals(&session(&server, &id));
    let card = cards.iter().find(|c| c["tool"] == "write_to_file").unwrap();
    let kept = card["args"]["CodeContent"].as_str().unwrap();
    assert!(kept.starts_with(&"x".repeat(4000)) && kept.contains("(1000 more characters)"));
    server.call(
        "POST",
        &format!("/api/approvals/{aid}/answer"),
        Some(json!({"decision": "allow", "always": true})),
    );
    assert_eq!(session(&server, &id)["meta"]["mode"], "accept-edits");
    let next = server.call(
        "POST",
        "/api/approvals",
        Some(json!({"session": id, "tool": "write_to_file", "args": {}})),
    );
    assert_eq!(next.json(), json!({"decision": "allow"}));
    server.stop();
}

#[test]
fn stop_declines_waiting_requests_and_wakes_the_waiting_hook() {
    let mut server = Server::new_stopped("approvals-stop", false);
    seed_rules(&server, json!({"allow": []}));
    fs::write(&server.script, json!({"delay_ms": 3000}).to_string()).unwrap();
    server.start();
    let id = server.create();
    server.call(
        "POST",
        &format!("/api/sessions/{id}/send"),
        Some(json!({"text": "Work for a while."})),
    );
    let aid = server
        .call(
            "POST",
            "/api/approvals",
            Some(json!({"session": id, "tool": "run_command", "args": {"CommandLine": "touch x"}})),
        )
        .json()["pending"]
        .as_str()
        .unwrap()
        .to_owned();
    let port = server.port;
    let token = server.token.clone();
    let waiter = thread::spawn(move || {
        let headers = std::collections::BTreeMap::from([("X-Antiquota-Token".into(), token)]);
        let started = Instant::now();
        let reply = crate::support::http::request(
            port,
            "GET",
            &format!("/api/approvals/{aid}?wait=4"),
            &headers,
            &[],
        );
        (reply.json(), started.elapsed())
    });
    thread::sleep(Duration::from_millis(300));
    server.call("POST", &format!("/api/sessions/{id}/stop"), Some(json!({})));
    let (reply, waited) = waiter.join().unwrap();
    assert_eq!(
        reply,
        json!({"decision": "deny", "reason": "The user stopped this session."})
    );
    assert!(
        waited < Duration::from_secs(3),
        "the waiting hook wakes at once"
    );
    let after = server.call(
        "POST",
        "/api/approvals",
        Some(json!({"session": id, "tool": "run_command", "args": {"CommandLine": "touch y"}})),
    );
    let after = after.json();
    assert!(
        after == json!({"decision": "deny", "reason": "The user stopped this session."})
            || after["pending"].is_string()
    );
    server.stop();
}

fn run_hook(server: &Server, kind: &str, env: &[(&str, &str)], input: &[u8]) -> (Value, Duration) {
    let mut command = Command::new(binary("antiquota"));
    command
        .args(["hook", kind])
        .env_remove("ANTIQUOTA_SESSION")
        .env_remove("ANTIQUOTA_MODE")
        .env("ANTIQUOTA_STATE", server.data.join("state"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    for (key, value) in env {
        command.env(key, value);
    }
    let started = Instant::now();
    let mut child = command.spawn().unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    (
        serde_json::from_slice(&output.stdout).unwrap(),
        started.elapsed(),
    )
}

#[test]
fn the_hook_fails_closed_and_starts_fast() {
    let server = Server::new_stopped("approvals-fallback", false);
    seed_rules(
        &server,
        json!({"allow": ["command(ls)"], "deny": ["command(rm)"]}),
    );
    let call = |line: &str| {
        json!({"toolCall": {"name": "run_command", "args": {"CommandLine": line}}})
            .to_string()
            .into_bytes()
    };
    let session = [("ANTIQUOTA_SESSION", "abc123")];
    // No engine runs: antiquota's own rules decide, and anything needing a yes is refused.
    let mut timings = vec![];
    for _ in 0..5 {
        let (allowed, elapsed) = run_hook(&server, "pre-tool", &session, &call("ls -la"));
        assert_eq!(allowed, json!({"decision": "allow"}));
        timings.push(elapsed);
    }
    let elapsed = *timings.iter().min().unwrap();
    let (denied, _) = run_hook(&server, "pre-tool", &session, &call("rm x"));
    assert_eq!(denied["decision"], "deny");
    let (asked, _) = run_hook(&server, "pre-tool", &session, &call("touch x"));
    assert_eq!(asked["decision"], "deny");
    assert!(asked["reason"].as_str().unwrap().contains("not running"));
    let (edit, _) = run_hook(
        &server,
        "pre-tool",
        &[
            ("ANTIQUOTA_SESSION", "abc123"),
            ("ANTIQUOTA_MODE", "accept-edits"),
        ],
        br#"{"toolCall":{"name":"write_to_file","args":{}}}"#,
    );
    assert_eq!(edit, json!({"decision": "allow"}));
    // Turns antiquota did not start are left to agy.
    let (neutral, _) = run_hook(&server, "pre-tool", &[], &call("rm x"));
    assert_eq!(neutral, json!({"decision": "ask"}));
    let (none, _) = run_hook(&server, "pre-invocation", &[], b"{}");
    assert_eq!(none, json!({}));
    let (malformed, _) = run_hook(&server, "pre-tool", &session, b"not json");
    assert_eq!(
        malformed["decision"], "allow",
        "an empty call is agy bookkeeping"
    );
    let (steer, _) = run_hook(&server, "pre-invocation", &session, b"{}");
    assert_eq!(steer, json!({}));
    let huge = vec![b' '; 1_000_001];
    let (oversized, _) = run_hook(&server, "pre-tool", &session, &huge);
    assert_eq!(oversized["decision"], "deny");
    assert!(
        elapsed < Duration::from_millis(500),
        "hook startup took {elapsed:?}"
    );
    server.evidence(
        "slice3-hook-startup",
        &json!({"local_decision_ms": timings.iter().map(|t| t.as_secs_f64() * 1000.0).collect::<Vec<_>>()}),
    );
}

#[test]
fn steering_reaches_agy_through_the_hook_with_mentioned_paths() {
    let mut server = Server::new_stopped("approvals-steer", false);
    fs::write(&server.script, json!({"delay_ms": 2500}).to_string()).unwrap();
    server.start();
    fs::write(server.project.join("notes.txt"), "notes").unwrap();
    let id = server.create();
    let send = |text: &str| {
        server
            .call(
                "POST",
                &format!("/api/sessions/{id}/send"),
                Some(json!({ "text": text })),
            )
            .json()["result"]
            .clone()
    };
    assert_eq!(send("Start working."), "started");
    assert_eq!(send("Also read @notes.txt please."), "queued");
    let (reply, _) = run_hook(
        &server,
        "pre-invocation",
        &[("ANTIQUOTA_SESSION", id.as_str())],
        b"{}",
    );
    let notes = server.project.join("notes.txt");
    assert_eq!(
        reply,
        json!({"injectSteps": [{"userMessage": format!(
            "Also read @notes.txt please.\n\nReferenced file:\n- {}",
            notes.display()
        )}]})
    );
    let (again, _) = run_hook(
        &server,
        "pre-invocation",
        &[("ANTIQUOTA_SESSION", id.as_str())],
        b"{}",
    );
    assert_eq!(again, json!({}), "a message is handed over once");
    let done = server.idle(&id);
    assert_eq!(done["meta"]["turns"], 1);
    let users: Vec<&Value> = done["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["t"] == "user")
        .collect();
    assert_eq!(users.len(), 2);
    assert_eq!(users[1]["steered"], true);
    assert_eq!(users[1]["text"], "Also read @notes.txt please.");
    server.stop();
}
