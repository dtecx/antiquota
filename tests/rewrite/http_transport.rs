//! Proves the HTTP boundary: exact local hosts on loopback, the GUI token file, body limits,
//! error mapping, discovery records, port fallback, the login PATH and shutdown signals.
use crate::support::{Server, binary, http};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream, UdpSocket},
    process::{Child, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

fn request(
    server: &Server,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> http::Response {
    let map: BTreeMap<String, String> = headers
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    http::request(server.port, method, path, &map, body)
}
fn authorized(server: &Server, method: &str, path: &str, body: &[u8]) -> http::Response {
    request(
        server,
        method,
        path,
        &[("X-Antiquota-Token", &server.token)],
        body,
    )
}
fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}
fn discovery(server: &Server) -> Value {
    serde_json::from_str(&fs::read_to_string(server.data.join("state/engine.json")).unwrap())
        .unwrap()
}

#[test]
fn only_exact_local_hosts_are_served_and_only_on_loopback() {
    let mut server = Server::new("http-hosts");
    let port = server.port;
    let hosts = [
        "localhost".to_owned(),
        "127.0.0.1".to_owned(),
        format!("127.0.0.1:{}", port.wrapping_add(1)),
        format!("localhost:{port}.evil.example"),
        format!("evil.example:{port}"),
        format!("[::1]:{port}"),
        format!("LOCALHOST:{port}"),
        format!("0.0.0.0:{port}"),
    ];
    for host in &hosts {
        for (path, token) in [("/api/health", false), ("/", false), ("/api/state", true)] {
            let mut headers = vec![("Host", host.as_str())];
            if token {
                headers.push(("X-Antiquota-Token", &server.token));
            }
            let response = request(&server, "GET", path, &headers, &[]);
            assert_eq!(response.status, 403, "{host} {path}");
            assert_eq!(
                response.json(),
                json!({"error": "bad host"}),
                "{host} {path}"
            );
        }
    }
    // The engine listens on loopback only: the machine's own outward address is refused.
    let outward = UdpSocket::bind("0.0.0.0:0").and_then(|socket| {
        socket.connect("192.0.2.1:9")?;
        socket.local_addr()
    });
    if let Ok(address) = outward
        && !address.ip().is_loopback()
        && !address.ip().is_unspecified()
    {
        let target = SocketAddr::new(address.ip(), port);
        assert!(
            TcpStream::connect_timeout(&target, Duration::from_millis(500)).is_err(),
            "{target} answered"
        );
    }
    server.stop();
}

#[test]
fn a_missing_gui_token_is_created_private_and_reused() {
    let mut server = Server::new_stopped("http-token", false);
    let file = server.data.join("state/gui-token");
    assert!(!file.exists());
    server.start();
    let first = fs::read_to_string(&file).unwrap();
    let token = first.trim();
    assert!(token.len() >= 32, "{token}");
    assert!(
        token
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    );
    assert_eq!(server.token, token);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    server.stop();
    server.start();
    assert_eq!(fs::read_to_string(&file).unwrap(), first);
    server.stop();
}

#[test]
fn request_bodies_have_exact_limits_and_loose_forms() {
    let mut server = Server::new("http-bodies");
    let id = server.create();
    let stop = format!("/api/sessions/{id}/stop");
    let send = format!("/api/sessions/{id}/send");
    let padded = |size: usize| {
        let mut body = br#"{"pad":""#.to_vec();
        body.resize(size - 2, b'x');
        body.extend(br#""}"#);
        assert_eq!(body.len(), size);
        body
    };
    // Exactly 5 MB is read; one byte more is refused.
    assert_eq!(
        authorized(&server, "POST", &stop, &padded(5_000_000)).status,
        200
    );
    // The engine answers from the declared length, before the body arrives.
    let mut socket = TcpStream::connect(("127.0.0.1", server.port)).unwrap();
    write!(
        socket,
        "POST {stop} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nX-Antiquota-Token: {}\r\nContent-Length: 5000001\r\nConnection: close\r\n\r\n",
        server.port, server.token
    )
    .unwrap();
    let mut reply = String::new();
    socket.read_to_string(&mut reply).unwrap();
    assert!(reply.starts_with("HTTP/1.1 400 "), "{reply}");
    assert!(reply.ends_with(r#"{"error":"body too large"}"#), "{reply}");
    // No body, and JSON that is not an object, read as `{}`; the legacy app failed on the latter.
    for body in [&b""[..], b"{}", b"[]", b"\"text\"", b"1", b"null"] {
        assert_eq!(authorized(&server, "POST", &stop, body).status, 200);
        let empty = authorized(&server, "POST", &send, body);
        assert_eq!(
            (empty.status, empty.json()),
            (400, json!({"error": "empty message"}))
        );
    }
    // Whitespace alone is malformed JSON, as Python's json.loads reports it.
    let blank = authorized(&server, "POST", &stop, b" ");
    assert_eq!(blank.status, 400);
    assert_eq!(
        blank.json(),
        json!({"error": "Expecting value: line 1 column 2 (char 1)"})
    );
    server.stop();
}

#[test]
fn errors_name_what_is_missing_and_what_is_invalid() {
    let mut server = Server::new("http-errors");
    let id = server.create();
    let not_found = |response: http::Response, what: &str| {
        assert_eq!(response.status, 404, "{what}");
        assert_eq!(
            response.json(),
            json!({"error": format!("not found: '{what}'")})
        );
    };
    not_found(authorized(&server, "GET", "/api/nowhere", &[]), "nowhere");
    not_found(
        authorized(&server, "GET", &format!("/api/sessions/{id}/nowhere"), &[]),
        &format!("sessions/{id}/nowhere"),
    );
    let ghost = "a0b1c2d3e4f5";
    for (method, path) in [
        ("GET", "git"),
        ("GET", "tree"),
        ("POST", "fork"),
        ("POST", "rewind"),
        ("POST", "stop"),
        ("POST", "term"),
    ] {
        not_found(
            authorized(
                &server,
                method,
                &format!("/api/sessions/{ghost}/{path}"),
                b"{}",
            ),
            ghost,
        );
    }
    // A missing file inside the session folder is "no such file", as in the legacy app.
    fs::write(server.project.join("present.txt"), "here").unwrap();
    let present = authorized(
        &server,
        "GET",
        &format!("/api/sessions/{id}/file?path=present.txt"),
        &[],
    );
    assert_eq!(
        (present.status, present.json()["text"].clone()),
        (200, json!("here"))
    );
    let missing = authorized(
        &server,
        "GET",
        &format!("/api/sessions/{id}/file?path=missing.txt"),
        &[],
    );
    let expected = antiquota_store::canonical(&server.project)
        .unwrap()
        .join("missing.txt");
    assert_eq!(missing.status, 404);
    assert_eq!(
        missing.json(),
        json!({"error": format!("no such file: {}", expected.to_string_lossy())})
    );
    // A file outside it is a validation error.
    let outside = authorized(
        &server,
        "GET",
        &format!("/api/sessions/{id}/file?path=../outside.txt"),
        &[],
    );
    assert_eq!(outside.status, 400);
    assert_eq!(
        outside.json(),
        json!({"error": "only files inside the session folder can be opened here"})
    );
    server.stop();
}

#[test]
fn discovery_records_every_field_and_ignores_a_stale_record() {
    let mut server = Server::new_stopped("http-discovery", false);
    // An engine that is gone left its record and holds no lock.
    let stale = free_port();
    fs::create_dir_all(server.data.join("state")).unwrap();
    fs::write(
        server.data.join("state/engine.json"),
        json!({"pid": 999_999, "port": stale, "url": format!("http://127.0.0.1:{stale}/"), "version": "0.0.1", "started": 1.0, "by": "cli"}).to_string(),
    )
    .unwrap();
    server.start();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();
    let info = discovery(&server);
    let mut keys: Vec<&str> = info
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort();
    assert_eq!(keys, ["by", "pid", "port", "started", "url", "version"]);
    assert_eq!(info["port"], json!(server.port));
    assert_eq!(
        info["url"],
        json!(format!("http://127.0.0.1:{}/", server.port))
    );
    assert_eq!(info["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(info["by"], "cli");
    let started = info["started"].as_f64().unwrap();
    assert!(started <= now && now - started < 60.0, "{started}");
    let health = authorized(&server, "GET", "/api/health", &[]).json();
    assert_eq!(info["pid"], health["pid"]);
    server.stop();
    assert!(!server.data.join("state/engine.json").exists());
    server
        .env
        .push(("ANTIQUOTA_LAUNCHED_BY".into(), "app".into()));
    server.start();
    assert_eq!(discovery(&server)["by"], "app");
    server.stop();
}

/// Starts `serve` with extra arguments and waits for its discovery record.
fn serve(server: &Server, args: &[&str]) -> (Child, Value) {
    let mut child = server
        .command()
        .args(["serve", "--no-open"])
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
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
fn shut_down(server: &Server, mut child: Child, port: u16) {
    let token = fs::read_to_string(server.data.join("state/gui-token")).unwrap();
    let headers = BTreeMap::from([("X-Antiquota-Token".to_owned(), token.trim().to_owned())]);
    assert_eq!(
        http::request(port, "POST", "/api/shutdown", &headers, b"{}").status,
        200
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    while child.try_wait().unwrap().is_none() {
        assert!(Instant::now() < deadline, "the engine did not stop");
        thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn an_occupied_port_moves_the_engine_to_a_free_one() {
    let server = Server::new_stopped("http-occupied", false);
    let taken = TcpListener::bind("127.0.0.1:0").unwrap();
    let busy = taken.local_addr().unwrap().port();
    let (child, info) = serve(&server, &["--port", &busy.to_string()]);
    let port = info["port"].as_u64().unwrap() as u16;
    assert_ne!(port, busy);
    assert_eq!(info["url"], json!(format!("http://127.0.0.1:{port}/")));
    let headers = BTreeMap::new();
    assert_eq!(
        http::request(port, "GET", "/api/health", &headers, &[]).status,
        200
    );
    shut_down(&server, child, port);
    drop(taken);
}

#[cfg(unix)]
#[test]
fn fix_path_takes_the_login_shells_path_within_five_seconds() {
    use std::os::unix::fs::PermissionsExt;
    let mut server = Server::new_stopped("http-fix-path", false);
    let shell = |name: &str, body: &str| {
        let path = server.root.join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        path.to_string_lossy().into_owned()
    };
    let login = shell(
        "login-shell",
        r#"printf 'welcome text\n\n__PATH__%s' '/opt/fixture-login/bin:/usr/bin:/bin'"#,
    );
    let slow = shell("slow-shell", "sleep 30");
    let path_of = |server: &Server| {
        let id = server.create();
        let body = json!({"cmd": r#"printf %s "$PATH""#}).to_string();
        authorized(
            server,
            "POST",
            &format!("/api/sessions/{id}/shell"),
            body.as_bytes(),
        )
        .json()["output"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    server.env = vec![
        ("SHELL".into(), login),
        ("ANTIQUOTA_FIX_PATH".into(), "1".into()),
    ];
    server.start();
    assert_eq!(path_of(&server), "/opt/fixture-login/bin:/usr/bin:/bin");
    server.stop();
    // A login shell that never answers costs at most five seconds and changes nothing.
    server.env = vec![
        ("SHELL".into(), slow),
        ("ANTIQUOTA_FIX_PATH".into(), "1".into()),
    ];
    let started = Instant::now();
    server.start();
    assert!(
        started.elapsed() < Duration::from_secs(9),
        "{:?}",
        started.elapsed()
    );
    let inherited = server
        .command()
        .get_envs()
        .find(|(key, _)| *key == "PATH")
        .and_then(|(_, value)| value.map(|v| v.to_string_lossy().into_owned()))
        .unwrap_or_else(|| std::env::var("PATH").unwrap());
    assert_eq!(path_of(&server), inherited);
    server.stop();
}

#[cfg(unix)]
#[test]
fn termination_and_interrupt_signals_stop_cleanly_and_leave_jobs_running() {
    for signal in ["TERM", "INT"] {
        let mut server = Server::new_stopped(&format!("http-signal-{signal}"), false);
        fs::write(&server.script, json!({"delay_ms": 20_000}).to_string()).unwrap();
        server.start();
        let id = server.create();
        let goal = json!({"objective": "Keep working.", "verifier": false}).to_string();
        assert_eq!(
            authorized(
                &server,
                "POST",
                &format!("/api/sessions/{id}/goal"),
                goal.as_bytes()
            )
            .status,
            200
        );
        let job = json!({"cmd": "sleep 30", "notify": false}).to_string();
        let started = authorized(
            &server,
            "POST",
            &format!("/api/sessions/{id}/jobs"),
            job.as_bytes(),
        )
        .json();
        let pid = started["pid"].as_u64().unwrap().to_string();
        let running = |pid: &str| {
            std::process::Command::new("kill")
                .args(["-0", pid])
                .status()
                .unwrap()
                .success()
        };
        let deadline = Instant::now() + Duration::from_secs(10);
        while authorized(&server, "GET", &format!("/api/sessions/{id}"), &[]).json()["meta"]["status"]
            != "running"
        {
            assert!(Instant::now() < deadline, "the goal turn did not start");
            thread::sleep(Duration::from_millis(20));
        }
        let engine = discovery(&server)["pid"].to_string();
        assert!(
            std::process::Command::new("kill")
                .args([format!("-{signal}"), engine.clone()])
                .status()
                .unwrap()
                .success()
        );
        let deadline = Instant::now() + Duration::from_secs(10);
        // The engine is this test's child, so it lingers as a zombie: watch its port instead.
        while server.data.join("state/engine.json").exists()
            || TcpStream::connect(("127.0.0.1", server.port)).is_ok()
        {
            assert!(
                Instant::now() < deadline,
                "SIG{signal} did not stop the engine"
            );
            thread::sleep(Duration::from_millis(20));
        }
        assert!(running(&pid), "the background job outlives the engine");
        // The lock is free again, and the session was paused, not lost.
        server.start();
        let session = authorized(&server, "GET", &format!("/api/sessions/{id}"), &[]).json();
        assert_eq!(session["meta"]["status"], "idle", "SIG{signal}");
        assert_eq!(session["meta"]["goal"]["state"], "paused", "SIG{signal}");
        let _ = std::process::Command::new("kill").arg(&pid).status();
        server.stop();
    }
}

#[test]
fn the_shared_stream_carries_account_and_session_events_and_pings_when_quiet() {
    let mut server = Server::new_stopped("http-shared-stream", false);
    // A declined tool ends the turn instead of starting a follow-up.
    fs::write(
        server.data.join("settings.json"),
        json!({"keep_awake": "off", "auto_continue": "0"}).to_string(),
    )
    .unwrap();
    fs::write(
        &server.script,
        json!({
            "scenario": "approval",
            "hook_command": [binary("antiquota"), "hook", "pre-tool"],
            "hook_payload": {"toolCall": {"name": "run_command", "args": {"CommandLine": "touch shared"}}},
        })
        .to_string(),
    )
    .unwrap();
    server.start();
    let mut events = http::Events::open(server.port, &server.token);
    let mut until = |what: &str, test: &dyn Fn(&Value) -> bool| loop {
        let event = events.next();
        if test(&event) {
            return event;
        }
        assert_ne!(event["t"], "deleted", "{what}");
    };
    // Account changes are global: they carry no session.
    let pause = json!({"on": true}).to_string();
    authorized(
        &server,
        "POST",
        "/api/accounts/one@example.com/pause",
        pause.as_bytes(),
    );
    let accounts = until("paused accounts", &|e| e["t"] == "accounts");
    assert_eq!(accounts["sid"], Value::Null);
    assert_eq!(accounts["accounts"][0]["paused"], true, "{accounts}");
    let resume = json!({"on": false}).to_string();
    authorized(
        &server,
        "POST",
        "/api/accounts/one@example.com/pause",
        resume.as_bytes(),
    );
    until("resumed accounts", &|e| {
        e["t"] == "accounts" && e["accounts"][0]["paused"] == false
    });
    // An approval request and its answer reach the stream with their session.
    let id = server.create();
    let send = json!({"text": "Use the tool."}).to_string();
    authorized(
        &server,
        "POST",
        &format!("/api/sessions/{id}/send"),
        send.as_bytes(),
    );
    let asked = until("approval", &|e| e["t"] == "approval");
    assert_eq!(
        (asked["sid"].as_str(), asked["state"].as_str()),
        (Some(id.as_str()), Some("pending"))
    );
    let answer = json!({"decision": "deny"}).to_string();
    authorized(
        &server,
        "POST",
        &format!("/api/approvals/{}/answer", asked["id"].as_str().unwrap()),
        answer.as_bytes(),
    );
    let answered = until("answer", &|e| {
        e["t"] == "approval" && e["state"] != "pending"
    });
    assert_eq!(answered["sid"], json!(id));
    until("turn end", &|e| e["t"] == "turn_end");
    // A background job announces its start and its end.
    let job = json!({"cmd": "printf done", "notify": false}).to_string();
    authorized(
        &server,
        "POST",
        &format!("/api/sessions/{id}/jobs"),
        job.as_bytes(),
    );
    let started = until("job start", &|e| e["t"] == "job");
    assert_eq!(
        (started["sid"].as_str(), started["phase"].as_str()),
        (Some(id.as_str()), Some("started"))
    );
    let ended = until("job end", &|e| e["t"] == "job" && e["phase"] == "ended");
    assert_eq!(ended["job"]["cmd"], "printf done");
    // A goal announces itself; stopping its turn ends this part.
    fs::write(&server.script, json!({"delay_ms": 20_000}).to_string()).unwrap();
    let goal = json!({"objective": "Watch the stream.", "verifier": false}).to_string();
    authorized(
        &server,
        "POST",
        &format!("/api/sessions/{id}/goal"),
        goal.as_bytes(),
    );
    let set = until("goal", &|e| e["t"] == "notice" && e["kind"] == "goal");
    assert_eq!(set["sid"], json!(id));
    assert_eq!(set["text"], "Goal set: Watch the stream.");
    until("goal turn", &|e| e["t"] == "turn_start");
    authorized(&server, "POST", &format!("/api/sessions/{id}/stop"), b"{}");
    until("stopped turn", &|e| e["t"] == "turn_end");
    // With nothing to say, the stream sends a ping comment every 15 seconds.
    let mut quiet = Instant::now();
    loop {
        let line = events
            .line_within(Duration::from_secs(20))
            .expect("the stream stays open");
        if line.starts_with("data: ") {
            quiet = Instant::now();
        } else if line == ": ping" {
            break;
        }
    }
    let waited = quiet.elapsed();
    assert!(
        waited >= Duration::from_secs(14) && waited < Duration::from_secs(18),
        "{waited:?}"
    );
    server.stop();
}
