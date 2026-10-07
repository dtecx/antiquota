//! Owns file-token account context and minimal account leases for turns.
use antiquota_core::{Account, AccountLease, AccountStore, PortError, Quota, Slot};
use antiquota_store::{FileLease, io_error, lock_file};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};
#[derive(Debug, Clone)]
pub struct FileAccounts {
    pub root: PathBuf,
}
struct Lease {
    slot: Slot,
    _file: FileLease,
}
impl AccountLease for Lease {
    fn slot(&self) -> &Slot {
        &self.slot
    }
}
fn slot_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 100
        && !name.starts_with('.')
        && name
            .bytes()
            .all(|x| x.is_ascii_alphanumeric() || b"@._+-".contains(&x))
}
impl FileAccounts {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
    fn slots(&self) -> Result<Vec<Slot>, PortError> {
        let path = self.root.join("slots");
        if !path.exists() {
            return Ok(vec![]);
        }
        let mut slots = vec![];
        for entry in fs::read_dir(path).map_err(io_error)? {
            let entry = entry.map_err(io_error)?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let auth = entry
                .path()
                .join(".gemini/antigravity-cli/antigravity-oauth-token");
            if slot_name(&name)
                && entry.file_type().map_err(io_error)?.is_dir()
                && auth.is_file()
                && !auth.is_symlink()
            {
                slots.push(Slot {
                    name,
                    home: entry.path().to_string_lossy().into_owned(),
                });
            }
        }
        slots.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(slots)
    }
    fn lease(&self, slot: Slot) -> Result<Option<Box<dyn AccountLease>>, PortError> {
        lock_file(
            &self
                .root
                .join("state/leases")
                .join(format!("{}.lock", slot.name)),
        )
        .map(|x| x.map(|file| Box::new(Lease { slot, _file: file }) as Box<dyn AccountLease>))
    }
}
impl AccountStore for FileAccounts {
    fn accounts(&self) -> Result<Vec<Account>, PortError> {
        let mut rows = vec![];
        for slot in self.slots()? {
            let busy = self.lease(slot.clone())?.is_none();
            let snapshot = read_json(
                &self
                    .root
                    .join("state/quota")
                    .join(format!("{}.json", slot.name)),
            )
            .unwrap_or(Value::Null);
            let quota = serde_json::from_str::<Quota>(
                &snapshot.get("quota").unwrap_or(&json!({})).to_string(),
            )
            .unwrap_or_default();
            let verify = read_json(
                &self
                    .root
                    .join("state/verify")
                    .join(format!("{}.json", slot.name)),
            );
            rows.push(Account {
                slot: slot.name.clone(),
                busy,
                probed: snapshot["at"].as_f64().unwrap_or(0.0),
                quota,
                walls: [("gemini".into(), 0.0), ("cg".into(), 0.0)].into(),
                why: [
                    ("gemini".into(), String::new()),
                    ("cg".into(), String::new()),
                ]
                .into(),
                paused: self.root.join("state/paused").join(&slot.name).exists(),
                verify,
                today: json!({}),
            });
        }
        Ok(rows)
    }
    fn acquire(&self, preferred: Option<&str>) -> Result<Option<Box<dyn AccountLease>>, PortError> {
        let mut slots = self.slots()?;
        if let Some(name) = preferred {
            slots.sort_by_key(|s| s.name != name);
        }
        for slot in slots {
            if self.root.join("state/paused").join(&slot.name).exists() {
                continue;
            }
            if let Some(lease) = self.lease(slot)? {
                return Ok(Some(lease));
            }
        }
        Ok(None)
    }
}
fn read_json(path: &Path) -> Option<Value> {
    fs::read(path)
        .ok()
        .and_then(|x| serde_json::from_slice(&x).ok())
}
#[cfg(test)]
mod tests;
