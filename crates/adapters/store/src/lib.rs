//! Owns compatible file persistence, account/engine locks, and usage projection.
mod attachments;
mod checkpoints;
mod files;
mod project_actions;
mod usage;
mod workspace;
use antiquota_core::{EventRecord, PortError, Session, SessionStore};
pub use files::*;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};
pub use workspace::FileWorkspace;
#[derive(Debug)]
pub struct FileStore {
    pub root: PathBuf,
    pub(crate) usage_writer: Mutex<()>,
}
impl FileStore {
    pub fn open(root: PathBuf) -> Result<Self, PortError> {
        fs::create_dir_all(root.join("sessions")).map_err(io_error)?;
        fs::create_dir_all(root.join("state")).map_err(io_error)?;
        Ok(Self {
            root,
            usage_writer: Mutex::new(()),
        })
    }
    pub fn session_dir(&self, sid: &str) -> Result<PathBuf, PortError> {
        if !valid_component(sid) {
            return Err(PortError::Invalid("invalid session id".into()));
        }
        let p = self.root.join("sessions").join(sid);
        if p.is_symlink() {
            return Err(PortError::Invalid(
                "session directory cannot be a symbolic link".into(),
            ));
        }
        Ok(p)
    }
    pub fn records(&self, sid: &str) -> Result<Vec<EventRecord>, PortError> {
        read_records(&self.session_dir(sid)?.join("events.jsonl"))
    }
    pub fn timestamp(
        &self,
        sid: &str,
        now: f64,
        records: &[EventRecord],
    ) -> Result<f64, PortError> {
        if !now.is_finite() || now < 0.0 {
            return Err(PortError::Invalid("invalid clock instant".into()));
        }
        let rows = read_values(&self.root.join("state/usage.jsonl"))?;
        let mut stamp = now;
        while records.iter().any(|r| r.ts == stamp)
            || rows
                .iter()
                .any(|r| r["session"] == sid && r["t"].as_f64() == Some(stamp))
        {
            stamp = stamp.next_up();
            if !stamp.is_finite() {
                return Err(PortError::Invalid(
                    "no finite turn timestamp remains".into(),
                ));
            }
        }
        Ok(stamp)
    }
}
impl SessionStore for FileStore {
    fn load(&self) -> Result<Vec<(Session, Vec<EventRecord>)>, PortError> {
        let mut loaded = vec![];
        for entry in fs::read_dir(self.root.join("sessions")).map_err(io_error)? {
            let entry = entry.map_err(io_error)?;
            let sid = entry.file_name().to_string_lossy().into_owned();
            if !entry.file_type().map_err(io_error)?.is_dir() || !valid_component(&sid) {
                continue;
            }
            let records = self.records(&sid)?;
            let snapshot = fs::read_to_string(entry.path().join("meta.json"))
                .ok()
                .and_then(|text| serde_json::from_str::<Session>(&text).ok());
            if snapshot.is_none()
                && !records
                    .iter()
                    .any(|r| matches!(r.event, antiquota_core::Event::Meta { .. }))
            {
                tracing::warn!(session=%sid,"unreadable session snapshot retained");
                continue;
            }
            let mut session = antiquota_core::replay(snapshot.unwrap_or_default(), &records)
                .map_err(|e| PortError::Invalid(e.to_string()))?;
            if !session.id.is_empty() && session.id != sid {
                tracing::warn!(session=%sid,"session identity mismatch retained");
                continue;
            }
            session.id = sid;
            loaded.push((session, records));
        }
        Ok(loaded)
    }
    fn append(&self, sid: &str, record: &EventRecord) -> Result<(), PortError> {
        let dir = self.session_dir(sid)?;
        fs::create_dir_all(&dir).map_err(io_error)?;
        append_json(&dir.join("events.jsonl"), record)
    }
    fn save(&self, session: &Session) -> Result<(), PortError> {
        let dir = self.session_dir(&session.id)?;
        fs::create_dir_all(&dir).map_err(io_error)?;
        write_json_atomic(&dir.join("meta.json"), session)
    }
    fn delete(&self, sid: &str) -> Result<(), PortError> {
        fs::remove_dir_all(self.session_dir(sid)?).map_err(io_error)
    }
    fn exists(&self, sid: &str) -> Result<bool, PortError> {
        Ok(self.session_dir(sid)?.exists())
    }
    fn validate_cwd(&self, cwd: &str) -> Result<String, PortError> {
        let home = user_home()?;
        let path = if cwd == "~" {
            home
        } else if let Some(rest) = cwd.strip_prefix("~/") {
            home.join(rest)
        } else {
            PathBuf::from(cwd)
        };
        let path = if path.is_absolute() {
            path
        } else {
            std::env::current_dir().map_err(io_error)?.join(path)
        };
        // Like os.path.abspath: drop `.` and fold `..` by text, without resolving links.
        let path = path.components().fold(PathBuf::new(), |mut out, part| {
            match part {
                std::path::Component::CurDir => {}
                std::path::Component::ParentDir => {
                    out.pop();
                }
                other => out.push(other),
            }
            out
        });
        if !path.is_dir() {
            return Err(PortError::Invalid(format!(
                "not a directory: {}",
                path.display()
            )));
        }
        Ok(path.to_string_lossy().into_owned())
    }
    fn attachments(
        &self,
        sid: &str,
        paths: &[String],
    ) -> Result<Vec<antiquota_core::Attachment>, PortError> {
        let Ok(dir) = self.session_dir(sid)?.join("attachments").canonicalize() else {
            return Ok(vec![]);
        };
        Ok(paths
            .iter()
            .filter_map(|path| {
                let canonical = Path::new(path).canonicalize().ok()?;
                if !canonical.starts_with(&dir) || !canonical.is_file() {
                    return None;
                }
                let name = canonical.file_name()?.to_string_lossy().into_owned();
                Some(antiquota_core::Attachment {
                    image: attachments::is_image(&name),
                    name,
                    path: canonical.to_string_lossy().into_owned(),
                    size: None,
                })
            })
            .collect())
    }
    fn turn_timestamp(
        &self,
        sid: &str,
        now: f64,
        records: &[EventRecord],
    ) -> Result<f64, PortError> {
        self.timestamp(sid, now, records)
    }
    fn project_usage(&self, sid: &str, records: &[EventRecord]) -> Result<(), PortError> {
        self.reconcile_usage(sid, records)
    }
    fn settings(&self) -> Result<Option<antiquota_core::Settings>, PortError> {
        read_settings(&self.root).map(Some)
    }
    fn save_settings(&self, settings: &antiquota_core::Settings) -> Result<(), PortError> {
        write_json_atomic(&self.root.join("settings.json"), settings)
    }
    fn save_state_document(&self, sid: &str, text: &str) -> Result<(), PortError> {
        write_atomic(
            &self.session_dir(sid)?.join("STATE.md"),
            format!("{text}\n").as_bytes(),
        )
    }
    fn checkpoint(
        &self,
        sid: &str,
        turn: u64,
        cwd: &str,
        path: &str,
        before: bool,
    ) -> Result<(), PortError> {
        self.capture_checkpoint(sid, turn, cwd, path, before)
    }
    fn rewind(
        &self,
        sid: &str,
        turn: u64,
        cwd: &str,
    ) -> Result<(Vec<String>, Vec<String>), PortError> {
        self.restore_checkpoints(sid, turn, cwd)
    }
    fn usage_history(
        &self,
        days: u32,
        now: f64,
        detail: bool,
    ) -> Result<serde_json::Value, PortError> {
        self.history(days, now, detail)
    }
    fn engine_details(&self) -> Result<serde_json::Value, PortError> {
        let read = |p: &str| {
            fs::read(self.root.join(p))
                .ok()
                .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                .unwrap_or(serde_json::json!({}))
        };
        let info = read("state/engine.json");
        let hooks = read("stack/.gemini/config/hooks.json");
        Ok(
            serde_json::json!({"url":info["url"],"by":info["by"],"started":info["started"],"base":self.root.to_string_lossy(),"stack":self.root.join("stack").to_string_lossy(),"log":self.root.join("state/engine.log").to_string_lossy(),"hook":hooks["antiquota"].is_object()&&hooks["antiquota"]["enabled"]!=false}),
        )
    }
    fn engine_log(&self) -> Result<String, PortError> {
        use std::io::{BufRead, BufReader};
        let file = match fs::File::open(self.root.join("state/engine.log")) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok("(no engine log: this engine was started from a terminal)".into());
            }
            Err(e) => return Err(io_error(e)),
        };
        let mut lines = std::collections::VecDeque::new();
        let mut input = BufReader::new(file);
        let mut bytes = vec![];
        while input.read_until(b'\n', &mut bytes).map_err(io_error)? > 0 {
            lines.push_back(String::from_utf8_lossy(&bytes).into_owned());
            bytes.clear();
            if lines.len() > 400 {
                lines.pop_front();
            }
        }
        Ok(lines.into_iter().collect())
    }
    fn state_dir(&self) -> String {
        self.root.join("state").to_string_lossy().into_owned()
    }
    fn save_attachment(
        &self,
        sid: &str,
        filename: &str,
        data: &[u8],
    ) -> Result<antiquota_core::Attachment, PortError> {
        self.store_attachment(sid, filename, data)
    }
    fn read_attachment(&self, sid: &str, name: &str) -> Result<Option<Vec<u8>>, PortError> {
        self.load_attachment(sid, name)
    }
    fn readable_dirs(&self, sid: &str) -> Vec<String> {
        self.existing_dirs(sid)
    }
    fn save_recording(&self, data: &[u8], extension: &str) -> Result<(String, String), PortError> {
        self.write_recording(data, extension)
    }
    fn discard_recording(&self, path: &str) {
        attachments::remove(path);
    }
    fn attachments_dir(&self, sid: &str) -> String {
        self.root
            .join("sessions")
            .join(sid)
            .join("attachments")
            .to_string_lossy()
            .into_owned()
    }
    fn worktrees_dir(&self) -> String {
        self.root.join("worktrees").to_string_lossy().into_owned()
    }
    fn job_log_path(&self, sid: &str, jid: &str) -> String {
        self.root
            .join("sessions")
            .join(sid)
            .join("jobs")
            .join(format!("{jid}.log"))
            .to_string_lossy()
            .into_owned()
    }
    fn resolve_mentions(&self, cwd: &str, candidates: &[String]) -> Vec<String> {
        let home = user_home().ok();
        let mut found: Vec<String> = vec![];
        for candidate in candidates {
            let expanded = match (candidate.strip_prefix('~'), &home) {
                (Some(rest), Some(home)) if rest.is_empty() || rest.starts_with('/') => {
                    home.join(rest.trim_start_matches('/'))
                }
                _ => std::path::PathBuf::from(candidate),
            };
            let path = normalize(&std::path::Path::new(cwd).join(expanded));
            let text = path.to_string_lossy().into_owned();
            if path.exists() && !found.contains(&text) {
                found.push(text);
            }
        }
        found
    }
}

/// Lexical normalization like the legacy engine's: `.` and `..` resolve without following links.
fn normalize(path: &std::path::Path) -> std::path::PathBuf {
    use std::path::Component;
    let mut out = std::path::PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}
pub fn valid_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}
pub fn io_error(error: std::io::Error) -> PortError {
    PortError::Unavailable(error.to_string())
}
pub fn read_records(path: &Path) -> Result<Vec<EventRecord>, PortError> {
    let text = match fs::read_to_string(path) {
        Ok(x) => x,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(io_error(e)),
    };
    let mut rows = vec![];
    for (number, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str(line) {
            Ok(row) => rows.push(row),
            Err(e) => {
                tracing::warn!(path=%path.display(),line=number+1,error=%e,"malformed event line retained")
            }
        }
    }
    Ok(rows)
}
#[cfg(test)]
mod tests;

pub fn read_settings(root: &Path) -> Result<antiquota_core::Settings, PortError> {
    match fs::read_to_string(root.join("settings.json")) {
        Ok(text) => Ok(settings_from(&text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Default::default()),
        Err(e) => Err(io_error(e)),
    }
}

/// Keeps every readable known setting, as the legacy reader did: a number or boolean a user
/// wrote by hand becomes its text, and one bad value never resets the others.
fn settings_from(text: &str) -> antiquota_core::Settings {
    use serde_json::Value;
    let Ok(Value::Object(mut map)) = serde_json::from_str::<Value>(text) else {
        tracing::warn!("settings are unreadable; using defaults without changing the file");
        return Default::default();
    };
    let mut known = serde_json::to_value(antiquota_core::Settings::default()).unwrap_or_default();
    // The default placement is left out of the wire format but is still a known key.
    known["placement"] = Value::String(antiquota_core::FILL.into());
    for key in known.as_object().into_iter().flat_map(|k| k.keys()) {
        match map.get(key) {
            Some(Value::Number(n)) => {
                let text = n.to_string();
                map.insert(key.clone(), Value::String(text));
            }
            Some(Value::Bool(b)) => {
                map.insert(
                    key.clone(),
                    Value::String(if *b { "True" } else { "False" }.into()),
                );
            }
            Some(Value::String(_)) | None => {}
            Some(_) => {
                map.remove(key);
            }
        }
    }
    // from_value cannot rebuild flattened arbitrary-precision numbers; reparsing the text can.
    serde_json::from_str(&Value::Object(map).to_string()).unwrap_or_default()
}
