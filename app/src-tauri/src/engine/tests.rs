//! Proves Rust sidecar selection and discovery without provider or desktop launches.
use super::*;
use serde_json::json;
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
};

struct Sandbox(PathBuf);
impl Sandbox {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let id = format!(
            "{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let path = antiquota_store::user_home()
            .unwrap()
            .join("Desktop/test/antiquota-desktop-engine")
            .join(id);
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn file(&self, relative: &str, bytes: &str) -> PathBuf {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes).unwrap();
        path
    }
    fn discovery(&self, value: Value, token: &str) {
        self.file("state/engine.json", &value.to_string());
        self.file("state/gui-token", token);
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn name() -> &'static str {
    if cfg!(windows) {
        "antiquota.exe"
    } else {
        "antiquota"
    }
}
fn env_value<'a>(command: &'a Command, key: &str) -> Option<&'a OsStr> {
    command
        .get_envs()
        .find(|(name, _)| *name == key)
        .and_then(|(_, value)| value)
}

#[test]
fn explicit_rust_engine_preserves_serve_and_selects_the_ui_override() {
    let sandbox = Sandbox::new();
    sandbox.file("ui/dist/index.html", "development UI");
    sandbox.file("custom-ui/index.html", "explicit UI");
    let explicit = sandbox.0.join("custom-engine");
    let ui = sandbox.0.join("custom-ui");
    let command = command_for(
        None,
        Some(explicit.as_os_str()),
        &sandbox.0,
        Some(ui.as_os_str()),
    )
    .unwrap();
    assert_eq!(command.get_program(), explicit.as_os_str());
    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        [OsStr::new("serve")]
    );
    assert_eq!(
        env_value(&command, "ANTIQUOTA_UI_DIR").map(Path::new),
        Some(ui.as_path())
    );
    assert_eq!(env_value(&command, "PYTHONPATH"), None);
    assert!(command_for(None, Some(OsStr::new("")), &sandbox.0, None).is_err());
}

#[test]
fn bundled_rust_sidecar_precedes_debug_then_release_and_uses_bundled_ui() {
    let sandbox = Sandbox::new();
    let resources = sandbox.0.join("resources");
    let bundled = sandbox.file(&format!("resources/engine/{}", name()), "fixture binary");
    let debug = sandbox.file(&format!("target/debug/{}", name()), "fixture debug binary");
    let release = sandbox.file(
        &format!("target/release/{}", name()),
        "fixture release binary",
    );
    sandbox.file("resources/ui/index.html", "bundled UI");
    sandbox.file("ui/dist/index.html", "development UI");
    for expected in [&bundled, &debug, &release] {
        let command = command_for(Some(&resources), None, &sandbox.0, None).unwrap();
        assert_eq!(Path::new(command.get_program()), expected.as_path());
        assert_eq!(
            env_value(&command, "ANTIQUOTA_UI_DIR").map(Path::new),
            Some(resources.join("ui").as_path())
        );
        fs::remove_file(expected).unwrap();
    }
    assert!(command_for(Some(&resources), None, &sandbox.0, None).is_err());
}

#[test]
fn development_requires_built_rust_engine_and_ui_without_python_fallback() {
    let sandbox = Sandbox::new();
    sandbox.file(
        "antiquota/__main__.py",
        "raise RuntimeError('must never execute')",
    );
    sandbox.file("ui/dist/index.html", "development UI");
    let error = command_for(None, None, &sandbox.0, None).unwrap_err();
    assert!(error.contains("Rust antiquota engine is missing"));
    let debug = sandbox.file(&format!("target/debug/{}", name()), "fixture binary");
    let command = command_for(None, None, &sandbox.0, None).unwrap();
    assert_eq!(Path::new(command.get_program()), debug.as_path());
    assert_eq!(
        env_value(&command, "ANTIQUOTA_UI_DIR").map(Path::new),
        Some(sandbox.0.join("ui/dist").as_path())
    );
    fs::remove_file(sandbox.0.join("ui/dist/index.html")).unwrap();
    assert!(
        command_for(None, None, &sandbox.0, None)
            .unwrap_err()
            .contains("UI is missing")
    );
}

struct Http {
    url: String,
    port: u16,
    requests: Arc<Mutex<Vec<String>>>,
    stopped: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Http {
    fn new(status: u16, body: Value, extra_headers: &str) -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        listener.set_nonblocking(true).unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured = requests.clone();
        let stopped = Arc::new(AtomicBool::new(false));
        let finished = stopped.clone();
        let body = body.to_string();
        let response = format!(
            "HTTP/1.1 {status} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{extra_headers}\r\n{body}",
            body.len()
        );
        let worker = thread::spawn(move || {
            while !finished.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut socket, _)) => {
                        // macOS inherits the listener's nonblocking mode on accepted sockets.
                        socket.set_nonblocking(false).unwrap();
                        socket
                            .set_read_timeout(Some(Duration::from_secs(1)))
                            .unwrap();
                        let mut request = Vec::new();
                        let mut chunk = [0; 4096];
                        loop {
                            let size = socket.read(&mut chunk).unwrap();
                            if size == 0 {
                                break;
                            }
                            request.extend_from_slice(&chunk[..size]);
                            if request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                                break;
                            }
                            assert!(request.len() < 8192);
                        }
                        captured
                            .lock()
                            .unwrap()
                            .push(String::from_utf8(request).unwrap());
                        socket.write_all(response.as_bytes()).unwrap();
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2))
                    }
                    Err(error) => panic!("fixture HTTP accept failed: {error}"),
                }
            }
        });
        Self {
            url: format!("http://127.0.0.1:{port}/"),
            port,
            requests,
            stopped,
            worker: Some(worker),
        }
    }
    fn info(&self, by: &str) -> Value {
        json!({"url":self.url,"port":self.port,"pid":1234,"by":by})
    }
    fn count(&self) -> usize {
        self.requests.lock().unwrap().len()
    }
}
impl Drop for Http {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
        let result = self.worker.take().unwrap().join();
        if !thread::panicking() {
            result.unwrap();
        }
    }
}
fn health_body() -> Value {
    json!({"ok":true,"app":"antiquota","pid":1234})
}

#[test]
fn discovery_matches_actual_health_pid_and_retains_external_engine_ownership() {
    let sandbox = Sandbox::new();
    let http = Http::new(200, health_body(), "");
    for (by, expected) in [("cli", false), ("app", true), ("unknown", false)] {
        sandbox.discovery(http.info(by), "  fixture-secret-token\n");
        let engine = discover(&sandbox.0).unwrap();
        assert_eq!(engine.url, http.url);
        assert_eq!(engine.token, "fixture-secret-token");
        assert_eq!(engine.by_app, expected);
        assert!(!format!("{engine:?}").contains("fixture-secret-token"));
    }
    let requests = http.requests.lock().unwrap();
    assert_eq!(requests.len(), 3);
    assert!(
        requests
            .iter()
            .all(|request| request.starts_with("GET /api/health HTTP/1.1"))
    );
    assert!(requests.iter().all(
        |request| !request.to_lowercase().contains("x-antiquota-token")
            && !request.contains("fixture-secret-token")
    ));
}

#[test]
fn malformed_discovery_foreign_origins_ports_and_empty_tokens_make_no_http_request() {
    let sandbox = Sandbox::new();
    let http = Http::new(200, health_body(), "");
    for url in [
        format!("http://localhost:{}/", http.port),
        format!("https://127.0.0.1:{}/", http.port),
        format!("http://127.0.0.1:{}/other/", http.port),
        format!("http://127.0.0.1:{}@example.invalid/", http.port),
        "http://example.invalid/".into(),
        "http://127.0.0.1:0/".into(),
    ] {
        let mut info = http.info("cli");
        info["url"] = json!(url);
        sandbox.discovery(info, "fixture-secret-token");
        assert!(discover(&sandbox.0).is_none());
        assert!(!healthy(&url));
    }
    for (field, value) in [
        ("port", json!(0)),
        ("port", json!(65536)),
        ("port", json!("bad")),
        ("pid", json!(0)),
        ("pid", json!("bad")),
        ("pid", json!(u64::MAX)),
    ] {
        let mut info = http.info("cli");
        info[field] = value;
        sandbox.discovery(info, "fixture-secret-token");
        assert!(discover(&sandbox.0).is_none());
    }
    sandbox.discovery(http.info("cli"), " \n\t");
    assert!(discover(&sandbox.0).is_none());
    sandbox.file("state/engine.json", "malformed JSON");
    assert!(discover(&sandbox.0).is_none());
    assert_eq!(http.count(), 0);
}

#[test]
fn stale_pid_nonhealthy_body_and_non_success_status_cannot_be_discovered() {
    for (status, body) in [
        (200, json!({"ok":true,"app":"antiquota","pid":4321})),
        (200, json!({"ok":false,"app":"antiquota","pid":1234})),
        (200, json!({"ok":true,"app":"other","pid":1234})),
        (200, json!({"ok":true,"app":"antiquota","pid":0})),
        (503, health_body()),
    ] {
        let sandbox = Sandbox::new();
        let http = Http::new(status, body, "");
        sandbox.discovery(http.info("app"), "fixture-secret-token");
        assert!(discover(&sandbox.0).is_none());
        assert_eq!(http.count(), 1);
    }
}

#[test]
fn redirects_never_forward_discovery_or_authenticated_requests() {
    let sandbox = Sandbox::new();
    let target = Http::new(200, health_body(), "");
    let redirect = Http::new(
        302,
        json!({}),
        &format!("Location: {}api/health\r\n", target.url),
    );
    sandbox.discovery(redirect.info("app"), "fixture-secret-token");
    assert!(discover(&sandbox.0).is_none());
    let engine = Engine {
        url: redirect.url.clone(),
        token: "fixture-secret-token".into(),
        by_app: false,
    };
    assert!(engine.get("api/state").is_none());
    assert!(!engine.post("api/shutdown"));
    assert_eq!(redirect.count(), 3);
    assert_eq!(target.count(), 0);
    let foreign = Engine {
        url: format!("http://localhost:{}/", target.port),
        ..engine
    };
    assert!(foreign.get("api/state").is_none());
    assert!(!foreign.post("api/shutdown"));
    assert_eq!(target.count(), 0);
}

#[test]
fn startup_timeout_handles_missing_and_empty_logs_without_fake_tail_lines() {
    let sandbox = Sandbox::new();
    for present in [false, true] {
        if present {
            sandbox.file("state/engine.log", "");
        }
        let message = startup_failure(&sandbox.0);
        assert!(message.contains("did not start within 30 seconds"));
        assert!(message.contains(&sandbox.0.join("state/engine.log").display().to_string()));
        assert_eq!(message.lines().count(), 1);
    }
}

#[test]
fn startup_timeout_reads_only_the_bounded_end_and_reports_the_last_twelve_lines() {
    let sandbox = Sandbox::new();
    let recent = (1..=20)
        .map(|index| format!("recent line {index}\n"))
        .collect::<String>();
    sandbox.file(
        "state/engine.log",
        &format!("{}\n{recent}", "old prefix".repeat(200_000)),
    );
    let message = startup_failure(&sandbox.0);
    let expected = (9..=20)
        .map(|index| format!("recent line {index}"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(message.split_once("\n\n").unwrap().1, expected);
    assert!(message.len() < 1024);
    assert!(!message.contains("old prefix"));
}

#[test]
fn startup_timeout_retains_readable_log_text_with_invalid_utf8() {
    let sandbox = Sandbox::new();
    let path = sandbox.file("state/engine.log", "");
    fs::write(path, b"first failure\ninvalid byte: \xff\nlast failure\n").unwrap();
    assert_eq!(
        startup_failure(&sandbox.0).split_once("\n\n").unwrap().1,
        "first failure\ninvalid byte: \u{fffd}\nlast failure"
    );
}

#[test]
fn startup_timeout_redacts_the_actual_gui_token_and_omits_tail_when_token_is_unreadable() {
    let sandbox = Sandbox::new();
    let token = "synthetic-private-gui-token";
    sandbox.file("state/gui-token", &format!(" \n{token}\n"));
    sandbox.file(
        "state/engine.log",
        &format!("header {token}\nurl ?token={token}\nfailure\n"),
    );
    let message = startup_failure(&sandbox.0);
    assert!(!message.contains(token));
    assert_eq!(
        message.split_once("\n\n").unwrap().1,
        "header [redacted]\nurl ?token=[redacted]\nfailure"
    );
    fs::write(sandbox.0.join("state/gui-token"), b"\xffprivate").unwrap();
    let message = startup_failure(&sandbox.0);
    assert!(!message.contains(token));
    assert_eq!(message.lines().count(), 1);
}

#[cfg(unix)]
#[test]
fn launch_configuration_appends_both_streams_and_sets_exact_owned_engine_arguments() {
    use std::os::unix::fs::PermissionsExt;
    let sandbox = Sandbox::new();
    let executable = sandbox.file("synthetic-sidecar", "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$ANTIQUOTA_DIR/arguments\"\nprintf '%s\\n' \"$ANTIQUOTA_LAUNCHED_BY\" \"$ANTIQUOTA_DIR\" > \"$ANTIQUOTA_DIR/ownership\"\nprintf 'new stdout\\n'\nprintf 'new stderr\\n' >&2\n");
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    sandbox.file("ui/dist/index.html", "synthetic UI");
    sandbox.file("state/engine.log", "previous engine output\n");
    let command = command_for(None, Some(executable.as_os_str()), &sandbox.0, None).unwrap();
    let mut command = launch_command(command, &sandbox.0).unwrap();
    assert_eq!(
        fs::metadata(sandbox.0.join("state/engine.log"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        [
            OsStr::new("serve"),
            OsStr::new("--port"),
            OsStr::new("8765")
        ]
    );
    assert_eq!(
        env_value(&command, "ANTIQUOTA_DIR"),
        Some(sandbox.0.as_os_str())
    );
    assert_eq!(
        env_value(&command, "ANTIQUOTA_LAUNCHED_BY"),
        Some(OsStr::new("app"))
    );
    assert!(command.status().unwrap().success());
    assert_eq!(
        fs::read_to_string(sandbox.0.join("arguments")).unwrap(),
        "serve\n--port\n8765\n"
    );
    assert_eq!(
        fs::read_to_string(sandbox.0.join("ownership")).unwrap(),
        format!("app\n{}\n", sandbox.0.display())
    );
    assert_eq!(
        fs::read_to_string(sandbox.0.join("state/engine.log")).unwrap(),
        "previous engine output\nnew stdout\nnew stderr\n"
    );
}

#[cfg(unix)]
#[test]
#[expect(
    clippy::zombie_processes,
    reason = "The fixture launcher must exit before its detached child to prove engine survival."
)]
fn detached_engine_survives_its_launcher_exit_in_an_independent_process_group() {
    const ROOT: &str = "ANTIQUOTA_DESKTOP_TEST_LAUNCH_ROOT";
    if let Some(base) = std::env::var_os(ROOT) {
        let base = PathBuf::from(base);
        let mut command = Command::new("/bin/sh");
        command.arg(base.join("synthetic-child.sh"));
        launch_command(command, &base).unwrap().spawn().unwrap();
        return;
    }
    let sandbox = Sandbox::new();
    sandbox.file("synthetic-child.sh", "printf '%s\\n' \"$$\" > \"$ANTIQUOTA_DIR/ready\"\nattempt=0\nwhile [ ! -f \"$ANTIQUOTA_DIR/release\" ] && [ \"$attempt\" -lt 200 ]; do\n  attempt=$((attempt + 1))\n  sleep 0.05\ndone\nif [ -f \"$ANTIQUOTA_DIR/release\" ]; then\n  printf 'output after launcher exit\\n'\n  printf 'done\\n' > \"$ANTIQUOTA_DIR/done\"\nfi\n");
    let status = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "engine::tests::detached_engine_survives_its_launcher_exit_in_an_independent_process_group"])
        .env(ROOT, &sandbox.0)
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
        .status().unwrap();
    assert!(status.success());
    let deadline = Instant::now() + Duration::from_secs(5);
    while !sandbox.0.join("ready").exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    let pid = fs::read_to_string(sandbox.0.join("ready")).unwrap();
    let group = Command::new("/bin/ps")
        .args(["-o", "pgid=", "-p", pid.trim()])
        .output()
        .unwrap();
    assert!(
        group.status.success(),
        "detached engine stopped with its launcher"
    );
    assert_eq!(String::from_utf8(group.stdout).unwrap().trim(), pid.trim());
    sandbox.file("release", "release synthetic child");
    while !sandbox.0.join("done").exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(sandbox.0.join("done").exists());
    assert_eq!(
        fs::read_to_string(sandbox.0.join("state/engine.log")).unwrap(),
        "output after launcher exit\n"
    );
}
