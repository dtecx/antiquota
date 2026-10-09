use super::*;
use std::{
    io::{BufRead, BufReader},
    net::TcpListener,
    sync::atomic::{AtomicU64, Ordering},
    thread,
};

static NEXT: AtomicU64 = AtomicU64::new(0);

/// A fresh data folder under the test sandbox, with its state folder.
fn data() -> PathBuf {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .expect("user home");
    let root = PathBuf::from(home)
        .join("Desktop/test/antiquota-cli-tests")
        .join(format!(
            "{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
    std::fs::create_dir_all(root.join("state")).unwrap();
    root
}

/// A one-connection-per-reply engine: each reply is `(status, body)`, and the request
/// heads it saw are returned when the thread ends.
fn engine(state: &Path, replies: Vec<(u16, Value)>) -> thread::JoinHandle<Vec<String>> {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::fs::write(
        state.join("engine.json"),
        json!({ "port": port }).to_string(),
    )
    .unwrap();
    std::fs::write(state.join("gui-token"), "fixture-token\n").unwrap();
    thread::spawn(move || {
        let mut seen = vec![];
        for (status, body) in replies {
            let (mut socket, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(socket.try_clone().unwrap());
            let (mut head, mut length) = (String::new(), 0);
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse().unwrap();
                }
                if line == "\r\n" {
                    break;
                }
                head.push_str(&line);
            }
            let mut body_in = vec![0; length];
            reader.read_exact(&mut body_in).unwrap();
            head.push_str(&String::from_utf8(body_in).unwrap());
            seen.push(head);
            let text = body.to_string();
            write!(
                socket,
                "HTTP/1.1 {status} X\r\nContent-Length: {}\r\n\r\n{text}",
                text.len()
            )
            .unwrap();
        }
        seen
    })
}

#[test]
fn tool_calls_come_from_each_payload_shape() {
    let shapes = [
        json!({"toolCall": {"name": "run_command", "args": {"CommandLine": "ls"}}}),
        json!({"toolCall": {"name": "run_command", "parameters": {"CommandLine": "ls"}}}),
        json!({"tool_name": "run_command", "tool_input": {"CommandLine": "ls"}}),
    ];
    for payload in shapes {
        assert_eq!(
            tool_call(&payload),
            ("run_command".into(), json!({"CommandLine": "ls"}))
        );
    }
    assert_eq!(tool_call(&json!({})), (String::new(), json!({})));
    assert_eq!(
        tool_call(&json!({"toolCall": {"name": "view_file", "args": "not an object"}})),
        ("view_file".into(), json!({}))
    );
}

#[test]
fn engine_answers_map_to_hook_decisions_and_lost_requests_are_refused() {
    assert_eq!(
        reply(&json!({"decision": "allow"})),
        Ok(json!({"decision": "allow"}))
    );
    assert_eq!(
        reply(&json!({"decision": "deny", "reason": "not now"})),
        Ok(deny("not now"))
    );
    assert_eq!(reply(&json!({"decision": "deny"})), Ok(deny(UNCHECKED)));
    assert_eq!(reply(&json!({"decision": "ask"})), Ok(deny(LOST)));
    assert!(reply(&json!({})).is_err());
}

#[test]
fn without_an_engine_the_hook_applies_the_owned_rules_and_refuses_questions() {
    let root = data();
    let settings = root.join("stack/.gemini/antigravity-cli/settings.json");
    std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
    std::fs::write(
        &settings,
        json!({"permissions": {"allow": ["command(ls)"], "deny": ["command(rm)"]}}).to_string(),
    )
    .unwrap();
    let state = root.join("state");
    let line = |command: &str| json!({ "CommandLine": command });
    assert!(matches!(
        ask_engine(&state, "s", "run_command", &line("ls")),
        Err(e) if e == "no running engine"
    ));
    assert_eq!(
        decide_here(&state, Mode::Default, "run_command", &line("ls -la")),
        json!({"decision": "allow"})
    );
    assert_eq!(
        decide_here(&state, Mode::Default, "run_command", &line("rm -rf build")),
        deny("A deny rule in antiquota's permissions blocks this command.")
    );
    assert_eq!(
        decide_here(&state, Mode::Default, "run_command", &line("touch notes")),
        deny(NO_APPROVER)
    );
    assert_eq!(
        decide_here(&state, Mode::Plan, "write_to_file", &json!({})),
        deny("Plan mode: describe the change instead of making it.")
    );
    // Without readable settings nothing is allowed by rule.
    let bare = data().join("state");
    assert_eq!(
        decide_here(&bare, Mode::Default, "run_command", &line("ls")),
        deny(NO_APPROVER)
    );
}

#[test]
fn discovery_needs_a_port_and_the_gui_token() {
    let state = data().join("state");
    assert!(matches!(Engine::find(&state), Err(e) if e == "no running engine"));
    std::fs::write(state.join("engine.json"), "{}").unwrap();
    assert!(matches!(Engine::find(&state), Err(e) if e == "engine discovery has no port"));
    std::fs::write(state.join("engine.json"), r#"{"port": 70000}"#).unwrap();
    assert!(matches!(Engine::find(&state), Err(e) if e == "engine discovery has no port"));
    std::fs::write(state.join("engine.json"), r#"{"port": 8765}"#).unwrap();
    assert!(matches!(Engine::find(&state), Err(e) if e == "engine token is unreadable"));
    std::fs::write(state.join("gui-token"), " secret \n").unwrap();
    let engine = Engine::find(&state).unwrap();
    assert_eq!((engine.port, engine.token.as_str()), (8765, "secret"));
}

#[test]
fn a_pending_request_is_polled_until_the_user_decides() {
    let state = data().join("state");
    let server = engine(
        &state,
        vec![
            (200, json!({"pending": "req-1"})),
            (200, json!({"pending": "req-1"})),
            (200, json!({"decision": "deny", "reason": "Not this file."})),
        ],
    );
    let args = json!({"CommandLine": "touch notes"});
    assert_eq!(
        ask_engine(&state, "session-1", "run_command", &args),
        Ok(deny("Not this file."))
    );
    let seen = server.join().unwrap();
    assert!(
        seen[0].starts_with("POST /api/approvals HTTP/1.1\r\n"),
        "{}",
        seen[0]
    );
    assert!(seen[0].contains("X-Antiquota-Token: fixture-token\r\n"));
    assert!(seen[0].ends_with(
        &json!({"session": "session-1", "tool": "run_command", "args": args}).to_string()
    ));
    for poll in &seen[1..] {
        assert!(
            poll.starts_with("GET /api/approvals/req-1?wait=25 HTTP/1.1\r\n"),
            "{poll}"
        );
    }
}

#[test]
fn an_engine_error_falls_back_instead_of_allowing() {
    let state = data().join("state");
    let server = engine(&state, vec![(403, json!({"error": "forbidden"}))]);
    assert_eq!(
        ask_engine(&state, "s", "run_command", &json!({})),
        Err("engine answered 403: forbidden".to_owned())
    );
    server.join().unwrap();
    let server = engine(&state, vec![(200, json!({"pending": null}))]);
    assert_eq!(
        ask_engine(&state, "s", "run_command", &json!({})),
        Err("engine reply has no decision".to_owned())
    );
    server.join().unwrap();
}
