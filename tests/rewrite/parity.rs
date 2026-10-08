//! Replays the remaining route captures and proves conversation and file boundaries.
use crate::{
    setup,
    support::{Server, replay::Replay},
};
use serde_json::{Value, json};
use std::fs;

#[test]
fn captured_settings_routes_keep_sources_read_only_and_reseed_keeps_the_hook() {
    let mut server = Server::new_stopped("parity-settings", true);
    setup::captured_home(&server);
    server
        .env
        .push(("ANTIQUOTA_LAUNCHED_BY".into(), "contract-capture".into()));
    server.start();
    let mut replay = Replay::new(&server);
    for case in [
        "engine",
        "engine-log",
        "filesystem",
        "filesystem-outside",
        "settings-file-missing",
        "settings-file-put",
        "settings-file-read",
        "settings-file-invalid",
        "settings-file-private",
        "settings-hook-put",
        "settings-hook-get",
        "settings-hook-disable",
        "settings-hook-delete",
        "settings-mcp-put",
        "settings-mcp-get",
        "settings-mcp-disable",
        "settings-mcp-delete",
        "settings-copy-rule",
        "settings-copy-skill",
        "settings-delete-rule",
        "settings-delete-skill",
        "settings-kind-error",
        "permissions-put",
        "stack",
    ] {
        replay.case(&server, case);
    }
    let old_hooks = fs::read(server.data.join("stack/.gemini/config/hooks.json")).unwrap();
    let state = server
        .data
        .join("stack/.gemini/antigravity-cli/conversations/kept");
    fs::write(&state, "keep history").unwrap();
    let reseed = server.call("POST", "/api/stack/reseed", Some(json!({})));
    assert_eq!(
        reseed.status,
        200,
        "{}",
        String::from_utf8_lossy(&reseed.body)
    );
    assert_eq!(
        reseed.json(),
        json!({"path":server.data.join("stack/.gemini").to_string_lossy(),"skills":[],"hooks":["antiquota"],"mcp":[]})
    );
    assert_eq!(
        old_hooks,
        fs::read(server.data.join("stack/.gemini/config/hooks.json")).unwrap()
    );
    assert_eq!(fs::read_to_string(state).unwrap(), "keep history");
    assert_eq!(
        fs::read_to_string(server.data.join("stack/.gemini/config/rules/example.md")).unwrap(),
        "Keep changes small.\n"
    );
    assert_eq!(
        fs::read_to_string(server.home.join(".gemini/config/hooks.json")).unwrap(),
        "{}"
    );
    assert!(server.home.join(".gemini/config/rules/example.md").exists());
    for (path, text) in [
        ("../outside", "x"),
        ("antigravity-cli/conversations/x", "x"),
        ("antigravity-cli/antigravity-oauth-token", "x"),
        (
            "antigravity-cli/settings.json",
            "{\"permissions\":{\"allow\":[\"command(python -c)\"]}}",
        ),
    ] {
        assert_eq!(
            server
                .call(
                    "PUT",
                    "/api/settings/file",
                    Some(json!({"path":path,"text":text}))
                )
                .status,
            400,
            "{path}"
        );
    }
    for path in [
        "./antigravity-cli/settings.json",
        "antigravity-cli/./settings.json",
    ] {
        assert_eq!(server.call("PUT", "/api/settings/file", Some(json!({"path":path,"text":"{\"permissions\":{\"allow\":[\"command(python -c)\"]}}"}))).status, 400);
    }
    let config = server.data.join("stack/.gemini/config/mcp_config.json");
    fs::write(&config, "{broken").unwrap();
    fs::write(
        server.home.join(".gemini/config/mcp_config.json"),
        "{\"mcpServers\":{\"fixture\":{}}}",
    )
    .unwrap();
    for (method, path, body) in [
        ("PUT", "/api/settings/mcp/new", Some(json!({"value":{}}))),
        ("DELETE", "/api/settings/mcp/new", None),
        (
            "POST",
            "/api/settings/copy",
            Some(json!({"source":"cli","kind":"mcp","name":"fixture"})),
        ),
    ] {
        assert_eq!(server.call(method, path, body).status, 400);
        assert_eq!(fs::read_to_string(&config).unwrap(), "{broken");
    }
    let settings = server.home.join(".gemini/antigravity-cli/settings.json");
    let unsafe_rules = "{\"permissions\":{\"allow\":[\"command(python -c)\"]}}";
    fs::write(&settings, unsafe_rules).unwrap();
    for kind in ["permissions", "everything"] {
        assert_eq!(
            server
                .call(
                    "POST",
                    "/api/settings/copy",
                    Some(json!({"source":"cli","kind":kind}))
                )
                .status,
            400
        );
    }
    assert_eq!(
        server
            .call("POST", "/api/stack/reseed", Some(json!({})))
            .status,
        400
    );
    assert_eq!(fs::read_to_string(settings).unwrap(), unsafe_rules);
    server.stop();
}
#[test]
fn captured_session_actions_preserve_transcript_and_inject_state_on_the_next_turn() {
    let mut server = Server::new_stopped("parity-sessions", false);
    setup::captured_home(&server);
    fs::write(
        server.project.join("readme.txt"),
        "Contract fixture project.\n",
    )
    .unwrap();
    server.start();
    let mut replay = Replay::new(&server);
    replay.case(&server, "session-create");
    replay.case(&server, "session-patch");
    let state = server.call("GET", "/api/state", None).json();
    let id = state["sessions"][0]["id"].as_str().unwrap().to_owned();
    replay.case(&server, "send");
    let first = server.idle(&id);
    assert_eq!(
        fs::read_to_string(server.data.join(format!("sessions/{id}/STATE.md"))).unwrap(),
        format!(
            "{}\n",
            server
                .call("GET", &format!("/api/sessions/{id}/state"), None)
                .json()["text"]
                .as_str()
                .unwrap()
        )
    );
    for case in [
        "search",
        "search-short",
        "session-state",
        "session-files",
        "session-note",
        "session-note-empty",
        "session-shell",
        "session-shell-empty",
        "session-compact",
        "session-fork",
        "session-inherit",
        "session-inherit-error",
        "session-rewind",
        "session-rewind-empty",
        "session-clear",
    ] {
        replay.case(&server, case);
    }
    let after = server
        .call("GET", &format!("/api/sessions/{id}"), None)
        .json();
    assert_eq!(after["meta"]["conversation_id"], Value::Null);
    assert_eq!(after["meta"]["context"], 0);
    assert_eq!(after["meta"]["pending_note"], "");
    assert_eq!(
        after["events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["t"] == "assistant")
            .count(),
        first["events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["t"] == "assistant")
            .count()
    );
    assert!(!server.project.join("notes.txt").exists());
    let other = after["meta"]["id"].as_str().unwrap();
    assert_eq!(other, id);
    server.call(
        "POST",
        &format!("/api/sessions/{id}/compact"),
        Some(json!({})),
    );
    setup::send(&server, &id, "Next request.");
    server.idle_turns(&id, 2);
    let calls = setup::turn_calls(&server);
    let next = setup::argument(calls.last().unwrap(), "-p").unwrap();
    assert!(next.contains("# Handoff: Contract renamed"));
    assert!(next.ends_with("Next request."));
    assert!(setup::argument(calls.last().unwrap(), "--conversation").is_none());
    let done = server
        .call("GET", &format!("/api/sessions/{id}"), None)
        .json();
    assert_eq!(done["meta"]["pending_note"], "");
    server.stop();
}
#[test]
fn conversation_mutations_refuse_busy_sessions_and_search_uses_the_latest_match() {
    let mut server = Server::new("parity-busy");
    let id = server.create();
    setup::script(&server, json!({"scenario":"delay","delay_ms":800}));
    setup::send(&server, &id, "First needle.");
    for action in ["clear", "compact", "rewind", "inherit"] {
        let response = server.call(
            "POST",
            &format!("/api/sessions/{id}/{action}"),
            Some(json!({"turn":1,"from":id})),
        );
        assert_eq!(response.status, 400, "{action}: {}", response.json());
        assert_eq!(response.json()["error"], "stop the running turn first");
    }
    server.idle(&id);
    setup::script(&server, json!({"reply":"Latest NEEDLE."}));
    setup::send(&server, &id, "Next request.");
    server.idle_turns(&id, 2);
    let hits = server.call("GET", "/api/search?q=NEEDLE", None).json();
    assert_eq!(hits.as_array().unwrap().len(), 1);
    assert_eq!(hits[0]["who"], "assistant");
    assert!(
        hits[0]["snippet"]
            .as_str()
            .unwrap()
            .contains("Latest NEEDLE.")
    );
    let before = server
        .call("GET", &format!("/api/sessions/{id}"), None)
        .json();
    server.stop();
    server.start();
    let after = server
        .call("GET", &format!("/api/sessions/{id}"), None)
        .json();
    assert_eq!(before["events"], after["events"]);
    server.stop();
}
#[test]
fn shell_merges_output_and_preserves_pending_notes_without_an_agent_call() {
    let mut server = Server::new("parity-shell");
    let id = server.create();
    server.call(
        "POST",
        &format!("/api/sessions/{id}/compact"),
        Some(json!({})),
    );
    let response=server.call("POST",&format!("/api/sessions/{id}/shell"),Some(json!({"cmd":"printf '\\033[31mfirst\\033[0m\\n'; printf 'second\\n' >&2; printf '%s' \"$NO_COLOR\""}))).json();
    assert_eq!(response["code"], 0);
    assert_eq!(response["output"], "first\nsecond\n1");
    assert!(response["ts"].as_f64().unwrap() > 0.0);
    let meta = server
        .call("GET", &format!("/api/sessions/{id}"), None)
        .json()["meta"]
        .clone();
    assert_eq!(meta["turns"], 0);
    assert!(
        meta["pending_note"]
            .as_str()
            .unwrap()
            .contains("# Handoff:")
    );
    assert!(
        meta["pending_note"]
            .as_str()
            .unwrap()
            .contains("first\nsecond\n1")
    );
    assert!(setup::turn_calls(&server).is_empty());
    let response = server
        .call(
            "POST",
            &format!("/api/sessions/{id}/shell"),
            Some(json!({"cmd":"printf '%060000d' 0; printf END"})),
        )
        .json();
    assert_eq!(response["output"].as_str().unwrap().chars().count(), 50_000);
    assert!(response["output"].as_str().unwrap().ends_with("END"));
    setup::send(&server, &id, "Continue.");
    server.idle(&id);
    let calls = setup::turn_calls(&server);
    let prompt = setup::argument(calls.last().unwrap(), "-p").unwrap();
    assert!(prompt.contains("# Handoff:"));
    assert!(prompt.contains("Output (last part)"));
    let tail = prompt
        .split("Output (last part):\n```\n")
        .last()
        .unwrap()
        .split("\n```")
        .next()
        .unwrap();
    assert_eq!(tail.chars().count(), 6000);
    assert!(prompt.contains("END\n```"));
    server.stop();
}

#[test]
fn usage_replays_captured_calendar_and_clamps_the_window() {
    let mut server = Server::new("parity-usage");
    let mut replay = Replay::new(&server);
    replay.case(&server, "usage-empty");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();
    let rows:Vec<_>=(0..6).map(|i|json!({"t":now,"slot":if i==5{"two@example.com"}else{"one@example.com"},"input":100,"output":10,"thinking":0,"model":"gemini-3.8-flash-high","session":"fixture"})).collect();
    let mut text = rows.iter().map(|r| format!("{r}\n")).collect::<String>();
    text.push_str("{broken\n");
    text.push_str(&format!(
        "{}\n",
        json!({"t":now+86400.0,"slot":"future","input":100})
    ));
    fs::write(server.data.join("state/usage.jsonl"), &text).unwrap();
    replay.case(&server, "usage-after-turns");
    for (days, count) in [(0, 1), (100, 90), (-2, 1)] {
        let response = server.call("GET", &format!("/api/usage?days={days}"), None);
        assert_eq!(response.status, 200);
        assert_eq!(response.json()["days"].as_array().unwrap().len(), count);
    }
    assert_eq!(server.call("GET", "/api/usage?days=no", None).status, 400);
    assert_eq!(
        fs::read_to_string(server.data.join("state/usage.jsonl")).unwrap(),
        text
    );
    let log = (0..405).map(|i| format!("line {i}\n")).collect::<String>();
    fs::write(server.data.join("state/engine.log"), log).unwrap();
    let tail = server.call("GET", "/api/engine/log", None).json()["text"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(tail.starts_with("line 5\n"));
    assert!(tail.ends_with("line 404\n"));
    assert_eq!(tail.lines().count(), 400);
    server.stop();
}

#[test]
fn stopped_sessions_can_run_shell_again_and_delete_waits_for_shell_cleanup() {
    use std::{
        thread,
        time::{Duration, Instant},
    };
    let mut server = Server::new("parity-shell-stop");
    let id = server.create();
    server.call("POST", &format!("/api/sessions/{id}/stop"), Some(json!({})));
    let response = server
        .call(
            "POST",
            &format!("/api/sessions/{id}/shell"),
            Some(json!({"cmd":"sleep 0.1; printf FINISHED"})),
        )
        .json();
    assert_eq!(response["code"], 0);
    assert_eq!(response["output"], "FINISHED");
    let port = server.port;
    let token = server.token.clone();
    let sid = id.clone();
    let marker = server.project.join("delayed.txt");
    let pending = thread::spawn(move || {
        let headers = std::collections::BTreeMap::from([
            ("X-Antiquota-Token".into(), token),
            ("Content-Type".into(), "application/json".into()),
        ]);
        crate::support::http::request(
            port,
            "POST",
            &format!("/api/sessions/{sid}/shell"),
            &headers,
            json!({"cmd":"sleep 2; printf bad > delayed.txt"})
                .to_string()
                .as_bytes(),
        )
    });
    thread::sleep(Duration::from_millis(200));
    let started = Instant::now();
    let deleted = server.call("DELETE", &format!("/api/sessions/{id}"), None);
    assert_eq!(deleted.status, 200);
    assert!(started.elapsed() < Duration::from_secs(1));
    assert_eq!(pending.join().unwrap().status, 404);
    thread::sleep(Duration::from_millis(2100));
    assert!(!marker.exists());
    assert!(!server.data.join(format!("sessions/{id}")).exists());
    server.stop();
}

#[test]
fn concurrent_project_notes_preserve_every_line_and_shutdown_cancels_shells() {
    use std::{
        collections::BTreeMap,
        thread,
        time::{Duration, Instant},
    };
    let mut server = Server::new("parity-note-shutdown");
    let ids = [server.create(), server.create()];
    fs::write(server.project.join("AGENTS.md"), "Existing instructions.").unwrap();
    thread::scope(|scope| {
        for n in 0..40 {
            let server = &server;
            let id = &ids[n % 2];
            scope.spawn(move || {
                assert_eq!(
                    server
                        .call(
                            "POST",
                            &format!("/api/sessions/{id}/note"),
                            Some(json!({"text":format!("Unique note {n}. ")}))
                        )
                        .status,
                    200
                )
            });
        }
    });
    let text = fs::read_to_string(server.project.join("AGENTS.md")).unwrap();
    assert!(text.starts_with("Existing instructions.\n"));
    for n in 0..40 {
        assert_eq!(
            text.lines()
                .filter(|line| *line == format!("- Unique note {n}."))
                .count(),
            1
        );
    }
    assert!(!server.project.join("GEMINI.md").exists());
    let port = server.port;
    let headers = BTreeMap::from([
        ("X-Antiquota-Token".into(), server.token.clone()),
        ("Content-Type".into(), "application/json".into()),
    ]);
    let id = ids[0].clone();
    let pending = thread::spawn(move || {
        crate::support::http::request(
            port,
            "POST",
            &format!("/api/sessions/{id}/shell"),
            &headers,
            json!({"cmd":"sleep 2; printf bad > late.txt"})
                .to_string()
                .as_bytes(),
        )
    });
    thread::sleep(Duration::from_millis(200));
    assert_eq!(
        server
            .call(
                "POST",
                &format!("/api/sessions/{}/rewind", ids[0]),
                Some(json!({"turn":1}))
            )
            .status,
        400
    );
    let started = Instant::now();
    server.stop();
    assert!(started.elapsed() < Duration::from_secs(1));
    pending.join().unwrap();
    thread::sleep(Duration::from_millis(2100));
    assert!(!server.project.join("late.txt").exists());
}
