//! Owns compatible file persistence, account/engine locks, and usage projection.
mod files;
mod usage;
use antiquota_core::{EventRecord, PortError, Session, SessionStore};
pub use files::*;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};
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
                Some(antiquota_core::Attachment {
                    name: canonical.file_name()?.to_string_lossy().into_owned(),
                    path: canonical.to_string_lossy().into_owned(),
                    image: matches!(
                        canonical.extension().and_then(|x| x.to_str()),
                        Some("png" | "jpg" | "jpeg" | "gif" | "webp")
                    ),
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
    fn state_dir(&self) -> String {
        self.root.join("state").to_string_lossy().into_owned()
    }
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
        Ok(text) => serde_json::from_str(&text).map_err(|e| PortError::Invalid(e.to_string())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Default::default()),
        Err(e) => Err(io_error(e)),
    }
}
