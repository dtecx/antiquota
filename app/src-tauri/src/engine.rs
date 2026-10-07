//! Finding, starting and talking to the antiquota engine (`antiquota serve`).
//!
//! The engine is a separate process: closing the app window, or quitting the app while work is
//! running, can leave it working in the background. A new app start attaches to it again
//! through `~/.antiquota/state/engine.json`, which the engine writes once it listens.

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct Engine {
    pub url: String,
    pub token: String,
    /// Started by this app (or an earlier run of it), so quitting the app may stop it.
    pub by_app: bool,
}

pub fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_default()
}

pub fn base_dir() -> PathBuf {
    match std::env::var_os("ANTIQUOTA_DIR") {
        Some(d) => PathBuf::from(d),
        None => home_dir().join(".antiquota"),
    }
}

fn state_file(name: &str) -> PathBuf {
    base_dir().join("state").join(name)
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(5)))
        .http_status_as_error(false)
        .build()
        .into()
}

fn info() -> Option<Value> {
    serde_json::from_str(&fs::read_to_string(state_file("engine.json")).ok()?).ok()
}

pub fn healthy(url: &str) -> bool {
    let Ok(mut r) = agent().get(&format!("{url}api/health")).call() else { return false };
    let Ok(body) = r.body_mut().read_to_string() else { return false };
    serde_json::from_str::<Value>(&body)
        .map(|v| v["app"] == "antiquota")
        .unwrap_or(false)
}

fn token() -> Option<String> {
    fs::read_to_string(state_file("gui-token")).ok().map(|t| t.trim().to_string())
}

/// The engine already running for this antiquota folder, if it answers.
pub fn running() -> Option<Engine> {
    let v = info()?;
    let url = v["url"].as_str()?.to_string();
    if !healthy(&url) {
        return None;
    }
    Some(Engine { url, token: token()?, by_app: v["by"] == "app" })
}

impl Engine {
    pub fn get(&self, path: &str) -> Option<Value> {
        let mut r = agent()
            .get(&format!("{}{}", self.url, path))
            .header("X-Antiquota-Token", &self.token)
            .call()
            .ok()?;
        if r.status() != 200 {
            return None;
        }
        serde_json::from_str(&r.body_mut().read_to_string().ok()?).ok()
    }

    pub fn post(&self, path: &str) -> bool {
        agent()
            .post(&format!("{}{}", self.url, path))
            .header("X-Antiquota-Token", &self.token)
            .header("Content-Type", "application/json")
            .send("{}")
            .map(|r| r.status() == 200)
            .unwrap_or(false)
    }
}

/// How to start the engine: a bundled `antiquotad`, an explicit one, or the source tree (dev).
fn engine_command(resources: Option<&Path>) -> Result<Command, String> {
    if let Some(exe) = std::env::var_os("ANTIQUOTA_ENGINE") {
        let mut c = Command::new(exe);
        c.arg("serve");
        return Ok(c);
    }
    let exe_name = if cfg!(windows) { "antiquotad.exe" } else { "antiquotad" };
    if let Some(dir) = resources {
        let bundled = dir.join("engine").join("antiquotad").join(exe_name);
        if bundled.is_file() {
            let mut c = Command::new(bundled);
            c.arg("serve");
            return Ok(c);
        }
    }
    // Development: run the engine from this repository with a Python new enough for it.
    let root = std::env::var_os("ANTIQUOTA_DEV_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."));
    if !root.join("antiquota").join("__main__.py").is_file() {
        return Err("The antiquota engine is missing from this build.".into());
    }
    let python = find_python().ok_or(
        "Python 3.10 or newer was not found. Install it, or set ANTIQUOTA_PYTHON to its path.",
    )?;
    let mut c = Command::new(python);
    c.args(["-m", "antiquota", "serve"]).current_dir(&root).env("PYTHONPATH", &root);
    Ok(c)
}

fn find_python() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = std::env::var_os("ANTIQUOTA_PYTHON").map(PathBuf::from).into_iter().collect();
    for p in [
        "/opt/homebrew/bin/python3",
        "/usr/local/bin/python3",
        "/Library/Frameworks/Python.framework/Versions/Current/bin/python3",
        "/usr/bin/python3",
    ] {
        candidates.push(PathBuf::from(p));
    }
    candidates.push(PathBuf::from(if cfg!(windows) { "python" } else { "python3" }));
    candidates.into_iter().find(|p| {
        Command::new(p)
            .args(["-c", "import sys; sys.exit(sys.version_info < (3, 10))"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    })
}

fn spawn(resources: Option<&Path>) -> Result<(), String> {
    let mut cmd = engine_command(resources)?;
    let log_path = state_file("engine.log");
    if let Some(dir) = log_path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let log = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|e| format!("cannot write {}: {e}", log_path.display()))?;
    let err = log.try_clone().map_err(|e| e.to_string())?;
    cmd.args(["--port", "8765"])
        .env("ANTIQUOTA_LAUNCHED_BY", "app")
        .env("ANTIQUOTA_FIX_PATH", "1") // an app gets a bare PATH; the engine asks the login shell
        .stdin(Stdio::null())
        .stdout(log)
        .stderr(err);
    detach(&mut cmd);
    cmd.spawn().map_err(|e| format!("could not start the engine: {e}"))?;
    Ok(())
}

#[cfg(unix)]
fn detach(cmd: &mut Command) {
    use std::os::unix::process::CommandExt;
    cmd.process_group(0); // its own group: the app quitting does not take it down
}

#[cfg(windows)]
fn detach(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
}

/// Attach to the running engine, or start one and wait until it answers.
pub fn ensure(resources: Option<&Path>) -> Result<Engine, String> {
    if let Some(e) = running() {
        return Ok(e);
    }
    spawn(resources)?;
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        if let Some(e) = running() {
            return Ok(e);
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    let tail = fs::read_to_string(state_file("engine.log"))
        .map(|t| t.lines().rev().take(12).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n"))
        .unwrap_or_default();
    Err(format!("The engine did not start within 30 seconds.\n\n{tail}"))
}
