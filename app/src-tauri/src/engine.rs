//! Finding, starting and talking to the antiquota engine (`antiquota serve`).
//!
//! The engine is a separate process: closing the app window, or quitting the app while work is
//! running, can leave it working in the background. A new app start attaches to it again
//! through `~/.antiquota/state/engine.json`, which the engine writes once it listens.

use serde_json::Value;
use std::ffi::OsStr;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct Engine {
    pub url: String,
    pub token: String,
    /// Started by this app (or an earlier run of it), so quitting the app may stop it.
    pub by_app: bool,
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("url", &self.url)
            .field("token", &"[redacted]")
            .field("by_app", &self.by_app)
            .finish()
    }
}

pub fn home_dir() -> PathBuf {
    antiquota_store::user_home().unwrap_or_default()
}

pub fn base_dir() -> PathBuf {
    antiquota_store::data_root().unwrap_or_else(|_| home_dir().join(".antiquota"))
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(5)))
        .http_status_as_error(false)
        .max_redirects(0)
        .proxy(None)
        .build()
        .into()
}

fn loopback_port(url: &str) -> Option<u16> {
    let port = url
        .strip_prefix("http://127.0.0.1:")?
        .strip_suffix('/')?
        .parse::<u16>()
        .ok()?;
    (port != 0 && url == format!("http://127.0.0.1:{port}/")).then_some(port)
}

fn health(url: &str) -> Option<Value> {
    loopback_port(url)?;
    let Ok(mut r) = agent().get(&format!("{url}api/health")).call() else {
        return None;
    };
    if r.status() != 200 {
        return None;
    }
    let body = r
        .body_mut()
        .with_config()
        .limit(65536)
        .read_to_string()
        .ok()?;
    let value: Value = serde_json::from_str(&body).ok()?;
    (value["app"] == "antiquota"
        && value["ok"] == true
        && value["pid"]
            .as_u64()
            .is_some_and(|pid| pid > 0 && pid <= u64::from(u32::MAX)))
    .then_some(value)
}

pub fn healthy(url: &str) -> bool {
    health(url).is_some()
}

/// The engine already running for this antiquota folder, if it answers.
pub fn running() -> Option<Engine> {
    discover(&base_dir())
}

fn discover(base: &Path) -> Option<Engine> {
    let state = base.join("state");
    let raw = fs::read_to_string(state.join("engine.json")).ok()?;
    let v: Value = serde_json::from_str(&raw).ok()?;
    let url = v["url"].as_str()?.to_string();
    let port = loopback_port(&url)?;
    let pid = v["pid"]
        .as_u64()
        .filter(|pid| *pid > 0 && *pid <= u64::from(u32::MAX))?;
    if v["port"].as_u64()? != u64::from(port) {
        return None;
    }
    let token = fs::read_to_string(state.join("gui-token"))
        .ok()?
        .trim()
        .to_string();
    if token.is_empty() {
        return None;
    }
    if health(&url)?["pid"].as_u64()? != pid {
        return None;
    }
    Some(Engine {
        url,
        token,
        by_app: v["by"] == "app",
    })
}

impl Engine {
    pub fn get(&self, path: &str) -> Option<Value> {
        loopback_port(&self.url)?;
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
        if loopback_port(&self.url).is_none() {
            return false;
        }
        agent()
            .post(&format!("{}{}", self.url, path))
            .header("X-Antiquota-Token", &self.token)
            .header("Content-Type", "application/json")
            .send("{}")
            .map(|r| r.status() == 200)
            .unwrap_or(false)
    }
}

/// Select the Rust engine and its UI resources without invoking a Python installation.
fn engine_command(resources: Option<&Path>) -> Result<Command, String> {
    let root = std::env::var_os("ANTIQUOTA_DEV_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."));
    command_for(
        resources,
        std::env::var_os("ANTIQUOTA_ENGINE").as_deref(),
        &root,
        std::env::var_os("ANTIQUOTA_UI_DIR").as_deref(),
    )
}

fn command_for(
    resources: Option<&Path>,
    explicit: Option<&OsStr>,
    root: &Path,
    ui_override: Option<&OsStr>,
) -> Result<Command, String> {
    let name = if cfg!(windows) {
        "antiquota.exe"
    } else {
        "antiquota"
    };
    let mut candidates = vec![];
    if let Some(resources) = resources {
        candidates.push(resources.join("engine").join(name));
    }
    candidates.extend([
        root.join("target/debug").join(name),
        root.join("target/release").join(name),
    ]);
    let executable = match explicit {
        Some(path) if !path.is_empty() => PathBuf::from(path),
        Some(_) => return Err("ANTIQUOTA_ENGINE is empty.".into()),
        None => candidates.into_iter().find(|path| path.is_file()).ok_or(
            "The Rust antiquota engine is missing. Build it with cargo build -p antiquota-cli.",
        )?,
    };
    let ui = ui_override
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            resources
                .map(|path| path.join("ui"))
                .filter(|path| path.join("index.html").is_file())
        })
        .unwrap_or_else(|| root.join("ui/dist"));
    if !ui.join("index.html").is_file() {
        return Err(
            "The antiquota UI is missing. Build ui/dist or include the bundled UI resources."
                .into(),
        );
    }
    let mut command = Command::new(executable);
    command.arg("serve").env("ANTIQUOTA_UI_DIR", ui);
    Ok(command)
}

fn spawn(resources: Option<&Path>) -> Result<(), String> {
    launch_command(engine_command(resources)?, &base_dir())?
        .spawn()
        .map_err(|e| format!("could not start the engine: {e}"))?;
    Ok(())
}

/// Configure an app-owned engine without changing the launching process's environment.
fn launch_command(mut cmd: Command, base: &Path) -> Result<Command, String> {
    let log_path = base.join("state/engine.log");
    if let Some(dir) = log_path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let log = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|e| format!("cannot write {}: {e}", log_path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        log.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("cannot protect {}: {error}", log_path.display()))?;
    }
    let err = log.try_clone().map_err(|e| e.to_string())?;
    cmd.args(["--port", "8765"])
        .env("ANTIQUOTA_LAUNCHED_BY", "app")
        .env("ANTIQUOTA_DIR", base)
        // Started from the Dock or a desktop menu, the app has a bare PATH; the engine asks the
        // login shell for the user's own (ignored on Windows).
        .env("ANTIQUOTA_FIX_PATH", "1")
        .stdin(Stdio::null())
        .stdout(log)
        .stderr(err);
    detach(&mut cmd);
    Ok(cmd)
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
    Err(startup_failure(&base_dir()))
}

/// Read only the bounded end of a failed launch's log, never exposing the GUI token.
fn startup_failure(base: &Path) -> String {
    let log_path = base.join("state/engine.log");
    let header = format!(
        "The engine did not start within 30 seconds. See {} for details.",
        log_path.display()
    );
    let tail = (|| -> std::io::Result<String> {
        let token = match fs::read_to_string(base.join("state/gui-token")) {
            Ok(token) => token,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(error),
        };
        let mut file = fs::File::open(&log_path)?;
        let length = file.metadata()?.len();
        const LIMIT: u64 = 65536;
        let start = length.saturating_sub(LIMIT);
        file.seek(SeekFrom::Start(start))?;
        let mut bytes = Vec::new();
        file.take(LIMIT).read_to_end(&mut bytes)?;
        // A truncated first line can contain part of a secret; exclude it entirely.
        let begin = if start > 0 {
            bytes
                .iter()
                .position(|byte| *byte == b'\n')
                .map_or(bytes.len(), |index| index + 1)
        } else {
            0
        };
        let text = String::from_utf8_lossy(&bytes[begin..]);
        let mut lines = text.lines().rev().take(12).collect::<Vec<_>>();
        lines.reverse();
        let tail = lines.join("\n");
        let token = token.trim();
        Ok(if token.is_empty() {
            tail
        } else {
            tail.replace(token, "[redacted]")
        })
    })()
    .unwrap_or_default();
    if tail.is_empty() {
        header
    } else {
        format!("{header}\n\n{tail}")
    }
}

#[cfg(test)]
mod tests;
