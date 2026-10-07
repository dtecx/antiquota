//! Proves the Terminal panel's shell routes and stream through the real engine.
use crate::support::{Server, http::Events, replay::Replay};
use serde_json::{Value, json};

fn decode(text: &str) -> Vec<u8> {
    let value = |c: u8| match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        _ => 63,
    };
    let clean: Vec<u8> = text.bytes().filter(|c| *c != b'=').collect();
    let mut out = vec![];
    for chunk in clean.chunks(4) {
        let n = chunk.iter().enumerate().fold(0u32, |acc, (i, c)| {
            acc | (u32::from(value(*c)) << (18 - 6 * i))
        });
        for i in 0..chunk.len().saturating_sub(1) {
            out.push((n >> (16 - 8 * i)) as u8);
        }
    }
    out
}

/// Reads frames until the decoded output contains `needle` or the stream reports an exit.
fn read_until(events: &mut Events, output: &mut String, needle: &str) -> bool {
    loop {
        let frame = events.next();
        if frame["exit"] == true {
            return false;
        }
        output.push_str(&String::from_utf8_lossy(&decode(
            frame["d"].as_str().unwrap_or(""),
        )));
        if output.contains(needle) {
            return true;
        }
    }
}

#[test]
fn captured_terminal_routes_and_stream() {
    let mut server = Server::new("terminal-captured");
    let id = server.create();
    let mut replay = Replay::new(&server);
    replay.bind_setup("{{SESSION_1}}", &id);
    replay.case(&server, "terminal-input-missing");
    replay.case(&server, "terminal-stream-missing");
    let shell = std::env::var("SHELL").unwrap_or_default();
    let created = if shell.ends_with("/zsh") {
        replay.case(&server, "terminal-create").json()
    } else {
        server
            .call(
                "POST",
                &format!("/api/sessions/{id}/term"),
                Some(json!({"cols": 80, "rows": 24})),
            )
            .json()
    };
    assert_eq!(created["alive"], true);
    assert_eq!(created["cwd"], server.project.to_string_lossy().as_ref());
    let again = server
        .call("POST", &format!("/api/sessions/{id}/term"), Some(json!({})))
        .json();
    assert_eq!(again, created, "a live shell is reused");
    let mut events = Events::open_path(
        server.port,
        &format!("/api/sessions/{id}/term/stream?token={}", server.token),
    );
    let first = events.next();
    assert_eq!(first["shell"], created["shell"]);
    assert_eq!(first["cwd"], created["cwd"]);
    assert_eq!(first["alive"], true);
    let mut output = String::from_utf8_lossy(&decode(first["d"].as_str().unwrap())).into_owned();
    replay.case(&server, "terminal-input");
    assert!(
        read_until(&mut events, &mut output, "contract-terminal\r\n"),
        "{output}"
    );
    replay.case(&server, "terminal-resize");
    let header_token = server.call(
        "GET",
        &format!("/api/sessions/{id}/term/stream?token=wrong"),
        None,
    );
    assert_eq!(header_token.status, 403, "the stream needs the query token");
    if shell.ends_with("/zsh") {
        replay.case(&server, "terminal-restart");
    } else {
        server.call(
            "POST",
            &format!("/api/sessions/{id}/term"),
            Some(json!({"restart": true})),
        );
    }
    let mut ended = String::new();
    assert!(
        !read_until(&mut events, &mut ended, "never"),
        "the restarted shell's stream ends"
    );
    let mut fresh = Events::open_path(
        server.port,
        &format!("/api/sessions/{id}/term/stream?token={}", server.token),
    );
    assert_eq!(fresh.next()["alive"], true);
    replay.case(&server, "terminal-kill");
    let mut gone = String::new();
    assert!(
        !read_until(&mut fresh, &mut gone, "never"),
        "kill ends the stream with exit"
    );
    assert!(gone.contains("[process exited]"));
    replay.case(&server, "terminal-kill-again");
    replay.case(&server, "terminal-input-missing");
    server.stop();
}

#[test]
fn an_exited_shell_refuses_input_until_restarted_and_deletion_kills_it() {
    let mut server = Server::new("terminal-exit");
    let id = server.create();
    server.call("POST", &format!("/api/sessions/{id}/term"), Some(json!({})));
    let mut events = Events::open_path(
        server.port,
        &format!("/api/sessions/{id}/term/stream?token={}", server.token),
    );
    events.next();
    server.call(
        "POST",
        &format!("/api/sessions/{id}/term/input"),
        Some(json!({"data": "exit\r"})),
    );
    let mut output = String::new();
    assert!(!read_until(&mut events, &mut output, "never"));
    let dead = server.call(
        "POST",
        &format!("/api/sessions/{id}/term/input"),
        Some(json!({"data": "ls\r"})),
    );
    assert_eq!(dead.status, 400);
    assert_eq!(dead.json()["error"], "the shell has exited: restart it");
    let resized = server.call(
        "POST",
        &format!("/api/sessions/{id}/term/resize"),
        Some(json!({"cols": 1, "rows": 1})),
    );
    assert_eq!(resized.json(), json!({"ok": true}));
    let restarted = server
        .call(
            "POST",
            &format!("/api/sessions/{id}/term"),
            Some(json!({"cols": 90})),
        )
        .json();
    assert_eq!(restarted["alive"], true);
    let mut live = Events::open_path(
        server.port,
        &format!("/api/sessions/{id}/term/stream?token={}", server.token),
    );
    live.next();
    server.call("DELETE", &format!("/api/sessions/{id}"), None);
    let mut rest = String::new();
    assert!(
        !read_until(&mut live, &mut rest, "never"),
        "deleting the session ends its shell"
    );
    let missing: Value = server
        .call(
            "POST",
            &format!("/api/sessions/{id}/term/kill"),
            Some(json!({})),
        )
        .json();
    assert_eq!(
        missing,
        json!({"ok": true}),
        "killing a missing shell is harmless"
    );
    server.stop();
}
