//! Isolated real server lifecycle, safe fake slot setup, and evidence collection.
pub mod http;
mod numbers;
pub mod replay;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
/// A command line for the platform shell: `sh` on POSIX, `cmd` on Windows.
pub fn shell_line<'a>(posix: &'a str, windows: &'a str) -> &'a str {
    if cfg!(windows) { windows } else { posix }
}
/// The line ending a command's output uses on this platform.
pub const NL: &str = if cfg!(windows) { "\r\n" } else { "\n" };
pub fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}
pub fn binary(name: &str) -> PathBuf {
    let extension = if cfg!(windows) { ".exe" } else { "" };
    let path = repo().join(format!("target/debug/{name}{extension}"));
    assert!(path.is_file(), "build {name} before integration tests");
    path
}

pub struct Server {
    pub root: PathBuf,
    pub home: PathBuf,
    pub data: PathBuf,
    pub project: PathBuf,
    pub script: PathBuf,
    pub agy: PathBuf,
    pub port: u16,
    pub token: String,
    /// Extra environment for the engine (and the fake agy it starts).
    pub env: Vec<(String, String)>,
    child: Option<Child>,
}
impl Server {
    pub fn new(label: &str) -> Self {
        Self::prepare(label, false, true)
    }
    pub fn new_stopped(label: &str, contract_accounts: bool) -> Self {
        Self::prepare(label, contract_accounts, false)
    }
    fn prepare(label: &str, contract_accounts: bool, start: bool) -> Self {
        let owner = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .unwrap();
        let root = PathBuf::from(owner)
            .join("Desktop")
            .join("test")
            .join("antiquota-rust-contract")
            .join(format!(
                "{label}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        let home = root.join("home");
        let data = root.join("data");
        let project = home.join("project");
        let script = root.join("fake-script.json");
        for path in [&home, &data, &project] {
            fs::create_dir_all(path).unwrap();
        }
        let slots: &[&str] = if contract_accounts {
            &["one@example.com", "two@example.com"]
        } else {
            &["one@example.com"]
        };
        for name in slots {
            let auth = data
                .join("slots")
                .join(name)
                .join(".gemini/antigravity-cli/antigravity-oauth-token");
            fs::create_dir_all(auth.parent().unwrap()).unwrap();
            fs::write(&auth, "synthetic-account-token").unwrap();
            private_file(&auth);
            if contract_accounts {
                let quota = data.join("state/quota").join(format!("{name}.json"));
                fs::create_dir_all(quota.parent().unwrap()).unwrap();
                let mut captured: Value = serde_json::from_slice(
                    &fs::read(repo().join("tests/fixtures/data/state-quota-one@example.com.json"))
                        .unwrap(),
                )
                .unwrap();
                captured["at"] = json!(
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs_f64()
                );
                fs::write(quota, captured.to_string()).unwrap();
            }
        }
        let agy = root.join(if cfg!(windows) {
            "fake-agy.exe"
        } else {
            "fake-agy"
        });
        fs::copy(binary("fake-agy"), &agy).unwrap();
        fs::write(data.join("settings.json"), "{\"keep_awake\":\"off\"}").unwrap();
        let ui = root.join("ui");
        fs::create_dir_all(&ui).unwrap();
        fs::write(ui.join("index.html"),"<html><meta name=\"antiquota-token\" content=\"__ANTIQUOTA_TOKEN__\"><body>Fixture UI</body></html>").unwrap();
        fs::write(ui.join("app.js"), "console.log('fixture');\n").unwrap();
        fs::write(&script, "{}").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let bin = root.join("browser-bin");
            fs::create_dir_all(&bin).unwrap();
            for name in ["open", "xdg-open"] {
                let path = bin.join(name);
                fs::write(
                    &path,
                    "#!/bin/sh\nprintf '%s\\n' \"$@\" >> \"$ANTIQUOTA_BROWSER_RECORD\"\n",
                )
                .unwrap();
                fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
            }
        }
        let mut server = Self {
            root,
            home,
            data,
            project,
            script,
            agy,
            port: 0,
            token: String::new(),
            env: Vec::new(),
            child: None,
        };
        if start {
            server.start();
        }
        server
    }
    pub fn start(&mut self) {
        let log = fs::File::create(self.root.join("server.log")).unwrap();
        let child = self
            .command()
            .args(["serve", "--port", "0", "--no-open"])
            .stdout(Stdio::from(log.try_clone().unwrap()))
            .stderr(Stdio::from(log))
            .spawn()
            .unwrap();
        self.child = Some(child);
        let deadline = Instant::now() + Duration::from_secs(12);
        loop {
            if let Ok(text) = fs::read_to_string(self.data.join("state/engine.json"))
                && let Ok(info) = serde_json::from_str::<Value>(&text)
                && info["pid"].as_u64() == Some(self.child.as_ref().unwrap().id().into())
            {
                self.port = info["port"].as_u64().unwrap() as u16;
                if let Ok(token) = fs::read_to_string(self.data.join("state/gui-token")) {
                    self.token = token.trim().to_string();
                    return;
                }
            }
            if self.child.as_mut().unwrap().try_wait().unwrap().is_some() {
                panic!(
                    "server exited: {}",
                    fs::read_to_string(self.root.join("server.log")).unwrap()
                );
            }
            assert!(Instant::now() < deadline, "server discovery timeout");
            thread::sleep(Duration::from_millis(20));
        }
    }
    pub fn command(&self) -> Command {
        let mut command = Command::new(binary("antiquota"));
        command
            .current_dir(&self.project)
            .env("ANTIQUOTA_HOME", &self.home)
            .env("ANTIQUOTA_DIR", &self.data)
            .env("ANTIQUOTA_AGY", &self.agy)
            .env("ANTIQUOTA_NO_USAGE_LINK", "1")
            .env("ANTIQUOTA_SIGNIN_PROBE", "online")
            .env("TOKIO_WORKER_THREADS", "2")
            .env(
                "ANTIQUOTA_BROWSER_RECORD",
                self.root.join("browser-record.txt"),
            )
            .env("ANTIQUOTA_UI_DIR", self.root.join("ui"))
            .env("FAKE_AGY_SCRIPT", &self.script)
            .env("FAKE_AGY_RECORD", self.root.join("fake-calls.jsonl"))
            .env_remove("FAKE_AGY_SCENARIO")
            .env_remove("STUB_SLEEP")
            .env_remove("FAKE_AGY_TRANSCRIPT");
        for (key, value) in &self.env {
            command.env(key, value);
        }
        #[cfg(unix)]
        {
            let inherited = std::env::var_os("PATH").unwrap_or_default();
            let paths = std::iter::once(self.root.join("browser-bin"))
                .chain(std::env::split_paths(&inherited));
            command.env("PATH", std::env::join_paths(paths).unwrap());
        }
        command
    }
    pub fn call(&self, method: &str, path: &str, body: Option<Value>) -> http::Response {
        let mut headers = BTreeMap::from([("X-Antiquota-Token".into(), self.token.clone())]);
        let bytes = body
            .map(|value| value.to_string().into_bytes())
            .unwrap_or_default();
        if !bytes.is_empty() {
            headers.insert("Content-Type".into(), "application/json".into());
        }
        http::request(self.port, method, path, &headers, &bytes)
    }
    pub fn create(&self) -> String {
        let response=self.call("POST","/api/sessions",Some(json!({"cwd":self.project,"title":"Contract fixture","model":"gemini-3.8-flash-high"})));
        assert_eq!(
            response.status,
            200,
            "{}",
            String::from_utf8_lossy(&response.body)
        );
        response.json()["id"].as_str().unwrap().to_string()
    }
    pub fn idle(&self, id: &str) -> Value {
        self.idle_turns(id, 1)
    }
    pub fn idle_turns(&self, id: &str, turns: u64) -> Value {
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            let value = self
                .call("GET", &format!("/api/sessions/{id}"), None)
                .json();
            if value["meta"]["status"] == "idle"
                && value["meta"]["turns"]
                    .as_u64()
                    .is_some_and(|count| count >= turns)
                && value["events"].as_array().is_some_and(|events| {
                    events.iter().any(|event| {
                        event["t"] == "turn_end"
                            && event["turn"].as_u64().is_some_and(|count| count >= turns)
                    })
                })
            {
                return value;
            }
            assert!(
                Instant::now() < deadline,
                "session did not become idle: {value}"
            );
            thread::sleep(Duration::from_millis(20));
        }
    }
    pub fn scenario(&self, scenario: &str) {
        fs::write(&self.script, json!({"scenario":scenario}).to_string()).unwrap();
    }
    pub fn stop(&mut self) {
        if self.child.is_some() {
            let response = self.call("POST", "/api/shutdown", Some(json!({})));
            assert_eq!(response.status, 200);
            assert_eq!(response.json(), json!({"ok":true}));
        }
        self.wait_stopped();
    }
    pub fn wait_stopped(&mut self) {
        if let Some(mut child) = self.child.take() {
            let deadline = Instant::now() + Duration::from_secs(5);
            while child.try_wait().unwrap().is_none() {
                if Instant::now() > deadline {
                    child.kill().unwrap();
                    child.wait().unwrap();
                    panic!("server shutdown timeout");
                }
                thread::sleep(Duration::from_millis(20));
            }
        }
    }
    pub fn evidence(&self, name: &str, value: &Value) {
        let dir = repo().join(".handoff/evidence");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join(format!("slice1-{name}.json")),
            serde_json::to_string_pretty(value).unwrap(),
        )
        .unwrap();
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn private_file(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    #[cfg(not(unix))]
    let _ = path;
}

/// Runs short CLI checks with a bounded wait, killing an unexpected hung process.
pub fn bounded_output(command: &mut Command) -> std::process::Output {
    bounded_output_with_timeout(command, Duration::from_secs(8))
}

pub fn bounded_output_with_timeout(
    command: &mut Command,
    timeout: Duration,
) -> std::process::Output {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + timeout;
    while child.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            child.kill().unwrap();
            let output = child.wait_with_output().unwrap();
            panic!(
                "process exceeded {timeout:?}:\n{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        thread::sleep(Duration::from_millis(20));
    }
    child.wait_with_output().unwrap()
}
