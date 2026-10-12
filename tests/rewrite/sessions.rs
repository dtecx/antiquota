//! Proves session creation, loading after restart and field updates through real HTTP.
use crate::support::Server;
use serde_json::{Value, json};
use std::fs;

#[test]
fn sessions_are_created_with_every_metadata_key_and_a_normalized_folder() {
    let mut server = Server::new("sessions-create");
    let created = server.call(
        "POST",
        "/api/sessions",
        Some(json!({"cwd": "~/project/../project/./", "title": "T", "model": "gemini-pro-high", "effort": "high", "mode": "plan"})),
    );
    assert_eq!(
        created.status,
        200,
        "{}",
        String::from_utf8_lossy(&created.body)
    );
    let meta = created.json();
    let id = meta["id"].as_str().unwrap();
    assert!(id.len() == 12 && id.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(meta["cwd"], server.project.to_string_lossy().as_ref());
    // The legacy route ignores effort on creation; it is set by a later update.
    assert_eq!(
        (
            meta["title"].clone(),
            meta["model"].clone(),
            meta["effort"].clone(),
            meta["mode"].clone()
        ),
        (
            json!("T"),
            json!("gemini-pro-high"),
            json!(""),
            json!("plan")
        )
    );
    let bare = server.call("POST", "/api/sessions", Some(json!({}))).json();
    assert_eq!(
        (
            bare["cwd"].clone(),
            bare["model"].clone(),
            bare["mode"].clone(),
            bare["title"].clone()
        ),
        (
            json!(server.home.to_string_lossy()),
            json!("gemini-3.8-flash-high"),
            json!("default"),
            json!("")
        )
    );
    let stored: Value = serde_json::from_str(
        &fs::read_to_string(server.data.join(format!("sessions/{id}/meta.json"))).unwrap(),
    )
    .unwrap();
    for key in [
        "id",
        "title",
        "cwd",
        "model",
        "effort",
        "mode",
        "created",
        "updated",
        "status",
        "status_until",
        "conversation_id",
        "turns",
        "queue",
        "goal",
        "last_slot",
        "usage",
        "context",
        "pending_note",
        "jobs",
        "loop",
        "pinned",
        "archived",
        "worktree",
    ] {
        assert!(stored.get(key).is_some(), "meta.json lacks {key}");
    }
    assert_eq!(
        stored["usage"],
        json!({"input": 0, "output": 0, "thinking": 0})
    );
    let missing = server.call("POST", "/api/sessions", Some(json!({"cwd": "~/nowhere"})));
    assert_eq!(missing.status, 400);
    assert_eq!(
        missing.json()["error"],
        format!("not a directory: {}", server.home.join("nowhere").display())
    );
    server.stop();
}

#[test]
fn restart_loads_valid_sessions_skips_broken_ones_and_fills_defaults() {
    let mut server = Server::new_stopped("sessions-load", false);
    let sessions = server.data.join("sessions");
    for (id, meta) in [
        ("aaaaaaaaaaaa", Some(json!({"id": "aaaaaaaaaaaa", "title": "Old", "cwd": server.project, "model": "gemini-3.8-flash-high", "status": "running", "turns": 3, "usage": {"input": 5, "output": 6},
            "conversation_id": "conv-kept", "queue": ["Queued before the crash."],
            "goal": {"objective": "Finish.", "check": "", "verifier": false, "verifier_model": "", "max_rounds": 20, "round": 2, "state": "active"},
            "loop": {"prompt": "Check in.", "every": 3600, "next": 4102444800.0}}).to_string())),
        ("bbbbbbbbbbbb", Some("{not json".to_owned())),
        ("cccccccccccc", None),
    ] {
        fs::create_dir_all(sessions.join(id)).unwrap();
        if let Some(meta) = meta {
            fs::write(sessions.join(id).join("meta.json"), meta).unwrap();
        }
    }
    server.start();
    let state = server.call("GET", "/api/state", None).json();
    let listed: Vec<&str> = state["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["id"].as_str().unwrap())
        .collect();
    assert_eq!(listed, ["aaaaaaaaaaaa"]);
    let old = &state["sessions"][0];
    assert_eq!(old["status"], "idle", "nothing runs after a restart");
    assert_eq!(
        (old["turns"].clone(), old["title"].clone()),
        (json!(3), json!("Old"))
    );
    assert_eq!(
        old["usage"],
        json!({"input": 5, "output": 6, "thinking": 0})
    );
    // Recoverable work is kept, and nothing starts by itself.
    assert_eq!(old["conversation_id"], "conv-kept");
    assert_eq!(old["queue"], json!(["Queued before the crash."]));
    // A goal interrupted by the restart is paused so the UI offers Resume.
    assert_eq!(
        (old["goal"]["state"].clone(), old["goal"]["round"].clone()),
        (json!("paused"), json!(2))
    );
    assert_eq!(old["loop"]["prompt"], "Check in.");
    std::thread::sleep(std::time::Duration::from_millis(500));
    let calls = fs::read_to_string(server.root.join("fake-calls.jsonl")).unwrap_or_default();
    assert!(
        !calls.contains("stream-json"),
        "no turn runs after a restart"
    );
    assert_eq!(
        (old["mode"].clone(), old["pinned"].clone()),
        (json!("default"), json!(false))
    );
    server.stop();
}

#[test]
fn updates_change_only_known_fields_and_coerce_flags() {
    let mut server = Server::new("sessions-update");
    let id = server.create();
    let patch = |body: Value| server.call("PATCH", &format!("/api/sessions/{id}"), Some(body));
    let before = patch(json!({})).json();
    let changed = patch(json!({"title": "New", "model": "gemini-pro-low", "effort": "low", "mode": "accept-edits", "pinned": "false", "archived": 1, "cwd": "/elsewhere", "turns": 9})).json();
    assert_eq!(
        (
            changed["title"].clone(),
            changed["model"].clone(),
            changed["effort"].clone(),
            changed["mode"].clone()
        ),
        (
            json!("New"),
            json!("gemini-pro-low"),
            json!("low"),
            json!("accept-edits")
        )
    );
    // Flags follow Python truthiness, so the string "false" pins.
    assert_eq!(
        (changed["pinned"].clone(), changed["archived"].clone()),
        (json!(true), json!(true))
    );
    assert_eq!(
        (changed["cwd"].clone(), changed["turns"].clone()),
        (before["cwd"].clone(), before["turns"].clone())
    );
    let cleared = patch(json!({"pinned": 0, "archived": []})).json();
    assert_eq!(
        (cleared["pinned"].clone(), cleared["archived"].clone()),
        (json!(false), json!(false))
    );
    // The saved file and the broadcast carry the same snapshot.
    let mut events = crate::support::http::Events::open(server.port, &server.token);
    let renamed = patch(json!({"title": "Broadcast"})).json();
    let announced = loop {
        let event = events.next();
        if event["t"] == "meta" && event["meta"]["title"] == "Broadcast" {
            break event;
        }
    };
    let stored: Value = serde_json::from_str(
        &fs::read_to_string(server.data.join(format!("sessions/{id}/meta.json"))).unwrap(),
    )
    .unwrap();
    assert_eq!(
        (announced["sid"].clone(), &announced["meta"]),
        (json!(id), &stored)
    );
    assert_eq!(renamed, stored);
    let refused = patch(json!({"title": "Not saved", "mode": "fast"}));
    assert_eq!(
        (refused.status, refused.json()["error"].clone()),
        (400, json!("unknown mode: fast"))
    );
    assert_eq!(
        patch(json!({})).json()["title"],
        "Broadcast",
        "a refused update changes nothing"
    );
    server.stop();
}

#[test]
fn deleting_a_running_session_stops_its_turn_removes_its_folder_and_announces_it() {
    use crate::support::http::Events;
    use std::time::{Duration, Instant};
    let mut server = Server::new("sessions-delete");
    let id = server.create();
    fs::write(
        &server.script,
        json!({"scenario": "delay", "delay_ms": 30_000}).to_string(),
    )
    .unwrap();
    let mut events = Events::open(server.port, &server.token);
    server.call(
        "POST",
        &format!("/api/sessions/{id}/send"),
        Some(json!({"text": "Long work."})),
    );
    while events.next()["t"] != "turn_start" {}
    let calls = server.root.join("fake-calls.jsonl");
    let deadline = Instant::now() + Duration::from_secs(10);
    let pid = loop {
        let found = fs::read_to_string(&calls)
            .unwrap_or_default()
            .lines()
            .rev()
            .find_map(|line| {
                let row: Value = serde_json::from_str(line).ok()?;
                row["argv"]
                    .as_array()?
                    .iter()
                    .any(|a| a == "stream-json")
                    .then(|| row["pid"].as_u64())?
            });
        if let Some(pid) = found {
            break pid;
        }
        assert!(
            Instant::now() < deadline,
            "the turn's agy process never started"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    let folder = server.data.join(format!("sessions/{id}"));
    assert!(folder.is_dir());
    assert_eq!(
        server
            .call("DELETE", &format!("/api/sessions/{id}"), None)
            .json(),
        json!({"ok": true})
    );
    let deleted = loop {
        let event = events.next();
        if event["t"] == "deleted" {
            break event;
        }
    };
    assert_eq!(deleted["sid"], id.as_str());
    assert!(!folder.exists(), "the session folder is removed");
    #[cfg(unix)]
    {
        let deadline = Instant::now() + Duration::from_secs(10);
        while std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .unwrap()
            .success()
        {
            assert!(
                Instant::now() < deadline,
                "agy process {pid} still runs after deletion"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    let _ = pid;
    server.stop();
}

#[test]
fn a_turn_survives_non_json_output_and_a_stderr_flood() {
    let mut server = Server::new("sessions-noise");
    let id = server.create();
    fs::write(&server.script, json!({"noise_bytes": 300_000}).to_string()).unwrap();
    server.call(
        "POST",
        &format!("/api/sessions/{id}/send"),
        Some(json!({"text": "Hello."})),
    );
    let done = server.idle(&id);
    let events = done["events"].as_array().unwrap();
    let end = events.iter().rev().find(|e| e["t"] == "turn_end").unwrap();
    assert_eq!(end["status"], "SUCCESS", "{end}");
    assert!(
        events.iter().any(|e| e["t"] == "assistant"),
        "the reply after the noise arrives"
    );
    assert!(
        done["meta"]["conversation_id"]
            .as_str()
            .is_some_and(|c| !c.is_empty()),
        "the init frame's conversation id is kept"
    );
    server.stop();
}

#[test]
fn the_final_result_response_is_the_reply_when_nothing_was_streamed() {
    use std::time::{Duration, Instant};
    let mut server = Server::new("sessions-result-reply");
    let id = server.create();
    let actions = json!([
        {"event": {"event": "init", "conversation_id": "result-only", "init": {"model": "stub"}}},
        {"event": {"event": "result", "result": {"conversation_id": "result-only", "status": "SUCCESS",
            "response": "Finished without streaming.\nGOAL_COMPLETE",
            "usage": {"input_tokens": 1, "output_tokens": 1, "total_tokens": 2}}}}
    ]);
    fs::write(&server.script, json!({"actions": actions}).to_string()).unwrap();
    let goal = server.call(
        "POST",
        &format!("/api/sessions/{id}/goal"),
        Some(json!({"objective": "Finish.", "verifier": false})),
    );
    assert_eq!(goal.status, 200, "{}", String::from_utf8_lossy(&goal.body));
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let meta = server
            .call("GET", &format!("/api/sessions/{id}"), None)
            .json()["meta"]
            .clone();
        if meta["goal"]["state"] == "met" {
            assert_eq!(meta["goal"]["round"], 1, "{meta}");
            break;
        }
        assert!(
            Instant::now() < deadline,
            "goal not met from result.response: {meta}"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    server.stop();
}

#[test]
fn a_lagging_event_reader_is_disconnected_instead_of_skipping_events() {
    use crate::support::http::Events;
    let mut server = Server::new("sessions-lag");
    let id = server.create();
    let mut events = Events::open_small(server.port, &server.token);
    // Far more meta snapshots than the 1024-event buffer while this reader reads nothing. They are
    // large, so a socket send buffer of even 16 MB (Linux autotunes up to 4 MB) holds only a few
    // hundred of them and the rest must wait in the event buffer.
    const ROUNDS: i64 = 2000;
    let title = "t".repeat(32_000);
    for round in 0..ROUNDS {
        server.call(
            "PATCH",
            &format!("/api/sessions/{id}"),
            Some(json!({"title": format!("{round} {title}")})),
        );
    }
    let mut received = 0;
    let mut last = -1;
    while let Some(event) = events.try_next() {
        if event["t"] == "meta" {
            let round: i64 = event["meta"]["title"]
                .as_str()
                .unwrap()
                .split(' ')
                .next()
                .unwrap()
                .parse()
                .unwrap();
            assert_eq!(
                round,
                last + 1,
                "events arrive in order with no gap before the stream ends"
            );
            last = round;
            received += 1;
        }
    }
    assert!(
        received < ROUNDS,
        "the lagging stream ended, so the client must reconnect and resync"
    );
    server.stop();
}

#[test]
fn with_every_account_paused_a_message_explains_the_wall_and_returns_idle() {
    let mut server = Server::new("sessions-all-paused");
    let id = server.create();
    let paused = server.call(
        "POST",
        "/api/accounts/one@example.com/pause",
        Some(json!({})),
    );
    assert_eq!(paused.status, 200);
    server.call(
        "POST",
        &format!("/api/sessions/{id}/send"),
        Some(json!({"text": "Work."})),
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
    let done = loop {
        let value = server
            .call("GET", &format!("/api/sessions/{id}"), None)
            .json();
        let walled = value["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["kind"] == "wall");
        if value["meta"]["status"] == "idle" && walled {
            break value;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "no wall explanation: {value}"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    let walls: Vec<&Value> = done["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["t"] == "notice" && e["kind"] == "wall")
        .collect();
    assert_eq!(walls.len(), 1, "{done}");
    assert_eq!(
        walls[0]["text"],
        "Every account with Gemini quota is paused (one@example.com). Resume one in the Accounts panel, then send again."
    );
    let calls = fs::read_to_string(server.root.join("fake-calls.jsonl")).unwrap_or_default();
    assert!(
        !calls.contains("stream-json"),
        "no turn runs while every account is paused"
    );
    server.stop();
}
