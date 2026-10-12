//! Proves terminal prompts, durable shared sessions, one-use approvals and cancellation.
use crate::support::{Server, binary, bounded_output};
use antiquota_pty::Console;
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

fn launch(server: &Server, args: &[&str]) -> Arc<Console> {
    let command = server.command();
    let mut env: BTreeMap<_, _> = command
        .get_envs()
        .filter_map(|(k, v)| {
            v.map(|v| {
                (
                    k.to_string_lossy().into_owned(),
                    v.to_string_lossy().into_owned(),
                )
            })
        })
        .collect();
    env.insert("NO_COLOR".into(), "1".into());
    env.insert("TERM".into(), "xterm-256color".into());
    Console::spawn(
        &binary("antiquota"),
        &args.iter().map(|s| (*s).into()).collect::<Vec<_>>(),
        &server.project,
        &env,
    )
    .unwrap()
}
fn wait(console: &Console, offset: usize, needle: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let output = console.text();
        if output.get(offset..).unwrap_or("").contains(needle) {
            return;
        }
        assert!(
            Instant::now() < deadline && !console.finished().unwrap(),
            "waiting for {needle}: {output}"
        );
        thread::sleep(Duration::from_millis(20));
    }
}
fn command(console: &Console, line: &str) {
    let offset = console.text().len();
    console.write(&format!("{line}\r")).unwrap();
    let echo = format!("you\r\n{line}\r\n");
    wait(console, offset, &echo);
    let start = console.text()[offset..].find(&echo).unwrap() + offset + echo.len();
    wait(console, start, "❯");
}
fn id(server: &Server) -> String {
    server.call("GET", "/api/state", None).json()["sessions"][0]["id"]
        .as_str()
        .unwrap()
        .into()
}
fn end(console: &Console) {
    console.write("\u{4}").unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !console.finished().unwrap() {
        assert!(Instant::now() < deadline, "CLI exit timed out");
        thread::sleep(Duration::from_millis(20));
    }
    assert!(
        console.text().contains("\u{1b}[?2004l"),
        "bracketed paste restored"
    );
}
fn evidence(label: &str, console: &Console) {
    let path = binary("antiquota")
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join(".handoff/evidence");
    fs::create_dir_all(&path).unwrap();
    fs::write(
        path.join(format!("slice23-{label}-pty.log")),
        console.text(),
    )
    .unwrap();
}

#[test]
fn terminal_sessions_stream_resume_and_share_controls_with_the_app() {
    let mut server = Server::new("interactive-session");
    fs::write(
        &server.script,
        json!({"reply":"Unique terminal reply λ.\u{1b}]52;c;private\u{7}","delay_ms":150})
            .to_string(),
    )
    .unwrap();
    for args in [
        vec!["init", "-q"],
        vec!["config", "user.email", "test@example.com"],
        vec!["config", "user.name", "Test"],
        vec!["config", "commit.gpgsign", "false"],
    ] {
        assert!(
            std::process::Command::new("git")
                .args(args)
                .current_dir(&server.project)
                .status()
                .unwrap()
                .success()
        );
    }
    fs::write(server.project.join("readme.txt"), "Original.\n").unwrap();
    for args in [vec!["add", "readme.txt"], vec!["commit", "-qm", "fixture"]] {
        assert!(
            std::process::Command::new("git")
                .args(args)
                .current_dir(&server.project)
                .status()
                .unwrap()
                .success()
        );
    }
    fs::write(
        server.project.join("readme.txt"),
        "Changed by terminal fixture.\n",
    )
    .unwrap();
    let discovery = fs::read(server.data.join("state/engine.json")).unwrap();
    let cli = launch(&server, &["-i", "--mode", "default"]);
    wait(&cli, 0, "❯");
    let sid = id(&server);
    command(&cli, "/rename Terminal harness");
    command(&cli, "/mode plan");
    let state = server
        .call("GET", &format!("/api/sessions/{sid}"), None)
        .json();
    assert_eq!(state["meta"]["mode"], "plan");
    command(&cli, "/mode default");
    command(&cli, "/model gemini-3.8-flash-high");
    command(&cli, "/diff");
    assert!(cli.text().contains("+Changed by terminal fixture."));
    cli.write("\u{1b}[200~A Unicode prompt λ.\nSecond line.\u{1b}[201~\r")
        .unwrap();
    server.idle(&sid);
    wait(&cli, 0, "Unique terminal reply λ.");
    let offset = cli.text().len();
    wait(&cli, offset.saturating_sub(8), "❯");
    let saved = server
        .call("GET", &format!("/api/sessions/{sid}"), None)
        .json();
    assert!(
        saved["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["t"] == "user" && e["text"] == "A Unicode prompt λ.\nSecond line.")
    );
    assert!(
        !cli.text().contains("\u{1b}]52"),
        "agent output cannot write to the terminal clipboard"
    );
    for line in [
        "/status",
        "/sessions",
        "/accounts",
        "/usage",
        "/model",
        "/help",
        "/missing",
        "/compact",
    ] {
        command(&cli, line);
    }
    let compacted = server
        .call("GET", &format!("/api/sessions/{sid}"), None)
        .json();
    assert!(compacted["meta"]["conversation_id"].is_null());
    assert!(
        !compacted["meta"]["pending_note"]
            .as_str()
            .unwrap()
            .is_empty()
    );
    assert!(cli.text().contains("unknown command /missing"));
    assert!(cli.text().contains("% quota"));
    assert!(cli.text().contains("0 active cooldowns"));
    for part in ["input", "output", "thinking", "cache"] {
        assert!(cli.text().contains(&format!("\"{part}\":")));
    }
    assert_eq!(
        compacted["meta"]["turns"], 1,
        "slash commands do not send a model turn"
    );
    assert!(
        !cli.text().contains("\u{1b}[33;1m"),
        "NO_COLOR keeps headings plain"
    );
    evidence("session", &cli);
    end(&cli);
    assert_eq!(
        fs::read(server.data.join("state/engine.json")).unwrap(),
        discovery,
        "discovered engine retained"
    );
    let cli = launch(&server, &["-i", "--resume", &sid]);
    wait(&cli, 0, "❯");
    assert!(cli.text().contains("Terminal harness"));
    // History restores the exact multiline user prompt from the durable session.
    cli.write("\u{1b}[A\r").unwrap();
    server.idle_turns(&sid, 2);
    wait(&cli, 0, "assistant · Ctrl+C");
    wait(&cli, cli.text().len().saturating_sub(8), "❯");
    command(&cli, "/fork");
    assert_eq!(
        server.call("GET", "/api/state", None).json()["sessions"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    command(&cli, "/new");
    assert_eq!(
        server.call("GET", "/api/state", None).json()["sessions"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    command(&cli, &format!("/resume {sid}"));
    evidence("resume", &cli);
    end(&cli);
    server.stop();
}

#[test]
fn terminal_approval_is_explicit_once_and_stop_keeps_the_session() {
    let mut server = Server::new_stopped("interactive-approval", false);
    fs::write(
        server.data.join("settings.json"),
        json!({"keep_awake":"off","auto_continue":"0"}).to_string(),
    )
    .unwrap();
    fs::write(&server.script, json!({"scenario":"approval","hook_command":[binary("antiquota"),"hook","pre-tool"],"hook_payload":{"toolCall":{"name":"run_command","args":{"CommandLine":"touch terminal-fixture"}}}}).to_string()).unwrap();
    server.start();
    let cli = launch(&server, &["-i"]);
    wait(&cli, 0, "❯");
    let sid = id(&server);
    let rules = server.call("GET", "/api/permissions", None).json();
    for (answer, state) in [("y\r", "allowed"), ("\r", "denied")] {
        let offset = cli.text().len();
        cli.write("Use the fixture tool.\r").unwrap();
        wait(&cli, offset, "[y/N]");
        let answer_offset = cli.text().len();
        cli.write(answer).unwrap();
        wait(&cli, answer_offset, "❯");
        let saved = server
            .call("GET", &format!("/api/sessions/{sid}"), None)
            .json();
        assert!(
            saved["events"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["t"] == "approval" && e["state"] == state)
        );
    }
    assert_eq!(server.call("GET", "/api/permissions", None).json(), rules);
    let offset = cli.text().len();
    cli.write("Use the fixture tool again.\r").unwrap();
    wait(&cli, offset, "[y/N]");
    let saved = server
        .call("GET", &format!("/api/sessions/{sid}"), None)
        .json();
    let aid = saved["events"]
        .as_array()
        .unwrap()
        .iter()
        .rev()
        .find(|e| e["t"] == "approval" && e["state"] == "pending")
        .unwrap()["id"]
        .as_str()
        .unwrap();
    let answer_offset = cli.text().len();
    assert_eq!(
        server
            .call(
                "POST",
                &format!("/api/approvals/{aid}/answer"),
                Some(json!({"decision":"deny","always":false}))
            )
            .status,
        200
    );
    wait(&cli, answer_offset, "❯");
    fs::write(
        &server.script,
        json!({"scenario":"delay","delay_ms":5000}).to_string(),
    )
    .unwrap();
    let offset = cli.text().len();
    cli.write("Please wait.\r").unwrap();
    wait(&cli, offset, "assistant · Ctrl+C");
    let stop_offset = cli.text().len();
    cli.write("\u{3}").unwrap();
    wait(&cli, stop_offset, "Stopping…");
    wait(&cli, stop_offset, "❯");
    // The prompt can return before the stopped turn end is saved under load; poll the saved state.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let saved = server
            .call("GET", &format!("/api/sessions/{sid}"), None)
            .json();
        let stopped = saved["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["t"] == "turn_end" && e["status"] == "stopped");
        if stopped && saved["meta"]["status"] == "idle" {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "stopped turn was not saved: {saved}"
        );
        thread::sleep(Duration::from_millis(20));
    }
    evidence("approval-stop", &cli);
    end(&cli);
    server.stop();
}

#[test]
fn interactive_pipe_rejection_happens_before_starting_an_engine() {
    let server = Server::new_stopped("interactive-pipe", false);
    let output = bounded_output(server.command().arg("-i"));
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("interactive mode needs a terminal"));
    assert!(!server.data.join("state/engine.json").exists());
}

#[test]
fn terminal_background_engine_survives_exit_and_no_accounts_keeps_the_message() {
    let mut server = Server::new_stopped("interactive-background", false);
    fs::remove_dir_all(server.data.join("slots")).unwrap();
    let cli = launch(&server, &["-i"]);
    wait(&cli, 0, "❯");
    let discovery: serde_json::Value =
        serde_json::from_slice(&fs::read(server.data.join("state/engine.json")).unwrap()).unwrap();
    server.port = discovery["port"].as_u64().unwrap() as u16;
    server.token = fs::read_to_string(server.data.join("state/gui-token"))
        .unwrap()
        .trim()
        .into();
    let sid = id(&server);
    let offset = cli.text().len();
    cli.write("Keep this request until I sign in.\r").unwrap();
    wait(&cli, offset, "assistant · Ctrl+C");
    let start = cli.text()[offset..].find("assistant · Ctrl+C").unwrap() + offset;
    wait(&cli, start, "❯");
    let saved = server
        .call("GET", &format!("/api/sessions/{sid}"), None)
        .json();
    assert_eq!(saved["meta"]["status"], "idle");
    assert!(
        saved["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["t"] == "user" && e["text"] == "Keep this request until I sign in.")
    );
    assert!(
        !server.root.join("fake-calls.jsonl").exists(),
        "no agy call without an account"
    );
    end(&cli);
    assert_eq!(
        server.call("GET", "/api/health", None).status,
        200,
        "background engine survives terminal exit"
    );
    assert_eq!(
        server.call("POST", "/api/shutdown", Some(json!({}))).status,
        200
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while server.data.join("state/engine.json").exists() {
        assert!(
            Instant::now() < deadline,
            "background engine did not shut down"
        );
        thread::sleep(Duration::from_millis(20));
    }
}

fn paused(server: &Server, slot: &str) -> bool {
    let overview = server.call("GET", "/api/accounts", None).json();
    let accounts = overview
        .as_array()
        .or_else(|| overview["accounts"].as_array())
        .unwrap()
        .clone();
    accounts.iter().find(|a| a["slot"] == slot).unwrap()["paused"]
        .as_bool()
        .unwrap()
}

#[test]
fn terminal_files_exports_and_account_controls_use_the_engine_api() {
    let mut server = Server::new("interactive-context");
    fs::write(
        &server.script,
        json!({"reply":"Read the staged file.","delay_ms":50}).to_string(),
    )
    .unwrap();
    fs::write(server.project.join("note λ.txt"), "Attached fixture.\n").unwrap();
    let cli = launch(&server, &["-i"]);
    wait(&cli, 0, "❯");
    let sid = id(&server);
    command(&cli, "/attach 'note λ.txt'");
    assert!(cli.text().contains("Staged note λ.txt (18 bytes)"));
    command(&cli, "/attach missing.txt");
    assert!(cli.text().contains("the attachment is unavailable"));
    command(&cli, "/attachments");
    assert!(cli.text().contains("1. note λ.txt (18 bytes)"));
    command(&cli, "/new");
    assert!(cli.text().contains("send the staged files or /detach all"));
    command(&cli, "/detach 2");
    assert!(cli.text().contains("use a number from /attachments"));
    let offset = cli.text().len();
    cli.write("Summarize the file.\r").unwrap();
    server.idle(&sid);
    wait(&cli, offset, "Read the staged file.");
    wait(&cli, cli.text().len().saturating_sub(8), "❯");
    assert!(cli.text()[offset..].contains("Attached: note λ.txt"));
    let saved = server
        .call("GET", &format!("/api/sessions/{sid}"), None)
        .json();
    let user = saved["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["t"] == "user" && e["text"] == "Summarize the file.")
        .unwrap()
        .clone();
    assert_eq!(user["attachments"][0]["name"], "note λ.txt");
    command(&cli, "/attachments");
    assert!(cli.text().contains("No files staged for the next message."));

    command(&cli, "/export export.json");
    let exported: serde_json::Value =
        serde_json::from_slice(&fs::read(server.project.join("export.json")).unwrap()).unwrap();
    assert_eq!(exported["meta"]["id"], sid);
    assert!(
        exported["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["t"] == "user")
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(server.project.join("export.json"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "exports are private");
    }
    fs::write(server.project.join("export.json"), "kept").unwrap();
    command(&cli, "/export export.json");
    assert!(cli.text().contains("existing files are never overwritten"));
    assert_eq!(
        fs::read_to_string(server.project.join("export.json")).unwrap(),
        "kept"
    );

    let slot = "one@example.com";
    command(&cli, &format!("/accounts pause {slot}"));
    assert!(paused(&server, slot));
    command(&cli, &format!("/accounts resume {slot}"));
    assert!(!paused(&server, slot));
    command(&cli, "/accounts pause ../x");
    command(&cli, "/accounts drop one@example.com");
    assert!(cli.text().contains("use /accounts check|pause|resume SLOT"));
    command(&cli, "/routing 0");
    assert!(
        cli.text()
            .contains("routing count must be between 1 and 200")
    );
    command(&cli, "/routing 5");
    let routing = server.call("GET", "/api/routing?n=5", None).json();
    if routing.as_array().unwrap().is_empty() {
        assert!(cli.text().contains("No recorded routing decisions."));
    } else {
        assert!(cli.text().contains(routing[0]["event"].as_str().unwrap()));
    }
    evidence("context", &cli);
    end(&cli);
    server.stop();
}
