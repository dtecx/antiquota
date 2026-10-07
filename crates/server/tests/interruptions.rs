//! Proves process cleanup and durable init metadata through the real HTTP engine.
#[allow(dead_code)]
#[path = "../../../tests/rewrite/support/http.rs"]
mod http;
#[path = "../../adapters/agy/tests/support/mod.rs"]
mod support;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
struct Engine {
    sandbox: support::Sandbox,
    child: Option<Child>,
    port: u16,
    token: String,
}
impl Engine {
    fn new(label: &str) -> Self {
        let sandbox = support::Sandbox::new(label);
        sandbox.token_home();
        let mut engine = Self {
            sandbox,
            child: None,
            port: 0,
            token: String::new(),
        };
        engine.start();
        engine
    }
    fn start(&mut self) {
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let executable = repo.join(if cfg!(windows) {
            "target/debug/antiquota.exe"
        } else {
            "target/debug/antiquota"
        });
        assert!(
            executable.is_file(),
            "build antiquota before HTTP integration tests"
        );
        let log = fs::File::create(self.sandbox.root.join("engine.log")).unwrap();
        self.child = Some(
            Command::new(executable)
                .args(["serve", "--port", "0", "--no-open"])
                .current_dir(&self.sandbox.root)
                .env("ANTIQUOTA_HOME", self.sandbox.root.join("home"))
                .env("ANTIQUOTA_DIR", &self.sandbox.root)
                .env("ANTIQUOTA_AGY", &self.sandbox.executable)
                .env("ANTIQUOTA_NO_USAGE_LINK", "1")
                .env_remove("FAKE_AGY_SCENARIO")
                .env_remove("FAKE_AGY_SCRIPT")
                .stdout(Stdio::from(log.try_clone().unwrap()))
                .stderr(Stdio::from(log))
                .spawn()
                .unwrap(),
        );
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            if let Ok(bytes) = fs::read(self.sandbox.root.join("state/engine.json"))
                && let Ok(info) = serde_json::from_slice::<Value>(&bytes)
                && info["pid"].as_u64() == Some(self.child.as_ref().unwrap().id().into())
            {
                self.port = info["port"].as_u64().unwrap() as u16;
                self.token = fs::read_to_string(self.sandbox.root.join("state/gui-token"))
                    .unwrap()
                    .trim()
                    .into();
                return;
            }
            assert!(
                self.child.as_mut().unwrap().try_wait().unwrap().is_none(),
                "engine exited before discovery"
            );
            assert!(Instant::now() < deadline, "engine discovery timeout");
            thread::sleep(Duration::from_millis(10));
        }
    }
    fn call(&self, method: &str, path: &str, body: Value) -> http::Response {
        let headers = BTreeMap::from([("X-Antiquota-Token".into(), self.token.clone())]);
        let response = http::request(
            self.port,
            method,
            path,
            &headers,
            body.to_string().as_bytes(),
        );
        assert_eq!(
            response.status,
            200,
            "{}",
            String::from_utf8_lossy(&response.body)
        );
        response
    }
    fn create(&self) -> String {
        self.call(
            "POST",
            "/api/sessions",
            json!({"cwd":self.sandbox.root,"model":"synthetic"}),
        )
        .json()["id"]
            .as_str()
            .unwrap()
            .into()
    }
    fn session(&self, id: &str) -> Value {
        self.call("GET", &format!("/api/sessions/{id}"), json!({}))
            .json()
    }
    fn idle(&self, id: &str, turns: u64) -> Value {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let value = self.session(id);
            if value["meta"]["status"] == "idle" && value["meta"]["turns"] == turns {
                return value;
            }
            assert!(
                Instant::now() < deadline,
                "turn did not finish promptly: {value}"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }
    fn wait_exit(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(3);
        let child = self.child.as_mut().unwrap();
        while child.try_wait().unwrap().is_none() {
            assert!(
                Instant::now() < deadline,
                "engine shutdown waited on descendant pipes"
            );
            thread::sleep(Duration::from_millis(10));
        }
        self.child.take();
    }
    fn crash(&mut self) {
        let mut child = self.child.take().unwrap();
        child.kill().unwrap();
        child.wait().unwrap();
        self.sandbox.kill_recorded("runner-pid");
    }
}
impl Drop for Engine {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
#[test]
fn stop_and_shutdown_finish_a_turn_with_inherited_descendant_pipes() {
    for action in ["stop", "shutdown"] {
        let mut engine = Engine::new(action);
        let id = engine.create();
        engine.call(
            "POST",
            &format!("/api/sessions/{id}/send"),
            json!({"text":"cancel"}),
        );
        engine.sandbox.wait_for("descendant-ready");
        assert_eq!(
            engine.session(&id)["meta"]["conversation_id"],
            "early-resume-id"
        );
        if action == "stop" {
            engine.call("POST", &format!("/api/sessions/{id}/stop"), json!({}));
            let finished = engine.idle(&id, 1);
            let ends: Vec<_> = finished["events"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|e| e["t"] == "turn_end")
                .collect();
            assert_eq!(ends.len(), 1);
            assert_eq!(ends[0]["status"], "stopped");
            assert_eq!(
                engine.call("GET", "/api/state", json!({})).json()["accounts"][0]["busy"],
                false
            );
        }
        engine.call("POST", "/api/shutdown", json!({}));
        engine.wait_exit();
        let events = fs::read_to_string(
            engine
                .sandbox
                .root
                .join("sessions")
                .join(&id)
                .join("events.jsonl"),
        )
        .unwrap();
        let ends: Vec<Value> = events
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .filter(|e| e["t"] == "turn_end")
            .collect();
        assert_eq!(ends.len(), 1);
        assert_eq!(ends[0]["status"], "stopped");
        thread::sleep(Duration::from_millis(2100));
        assert!(
            !engine.sandbox.root.join("descendant-wrote").exists(),
            "{action}: descendant wrote after completion"
        );
    }
}
#[test]
fn abrupt_engine_exit_after_init_resumes_the_durable_conversation() {
    let mut engine = Engine::new("init-restart");
    let id = engine.create();
    engine.call(
        "POST",
        &format!("/api/sessions/{id}/send"),
        json!({"text":"init-only"}),
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    let before = loop {
        let value = engine.session(&id);
        if value["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["t"] == "tool" && e["state"] == "done")
        {
            break value;
        }
        assert!(Instant::now() < deadline, "missing durable post-init frame");
        thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(before["meta"]["conversation_id"], "early-resume-id");
    assert!(
        !before["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["t"] == "turn_end")
    );
    engine.crash();
    engine.start();
    let after = engine.session(&id);
    assert_eq!(after["meta"]["conversation_id"], "early-resume-id");
    assert_eq!(after["meta"]["status"], "idle");
    engine.call(
        "POST",
        &format!("/api/sessions/{id}/send"),
        json!({"text":"resume"}),
    );
    let finished = engine.idle(&id, 2);
    assert_eq!(finished["meta"]["conversation_id"], "early-resume-id");
    let args = fs::read_to_string(engine.sandbox.root.join("runner-args")).unwrap();
    assert!(
        args.contains("--conversation\nearly-resume-id"),
        "restart did not pass durable resume ID: {args}"
    );
    engine.call("POST", "/api/shutdown", json!({}));
    engine.wait_exit();
}
