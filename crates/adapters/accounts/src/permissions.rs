//! Owns antiquota's permission lists and hook entry inside the own stack's agy configuration.
use crate::{FileAccounts, files::read_json};
use antiquota_core::{Permissions, PortError, hooks::install_hook_entry};
use antiquota_store::write_json_atomic;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn settings_path(accounts: &FileAccounts) -> PathBuf {
    accounts
        .root
        .join("stack/.gemini/antigravity-cli/settings.json")
}

fn hooks_path(accounts: &FileAccounts) -> PathBuf {
    accounts.root.join("stack/.gemini/config/hooks.json")
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

/// The allow and deny lists; a missing file or list reads as empty.
pub fn read(accounts: &FileAccounts) -> Permissions {
    let settings = read_json(&settings_path(accounts)).unwrap_or_default();
    Permissions {
        allow: strings(&settings["permissions"]["allow"]),
        deny: strings(&settings["permissions"]["deny"]),
    }
}

/// Replaces the given lists, keeping every other setting and permission key as it was.
/// Returns the stored permission object.
pub fn save(
    accounts: &FileAccounts,
    allow: Option<Vec<String>>,
    deny: Option<Vec<String>>,
) -> Result<Value, PortError> {
    let _guard = accounts
        .changes
        .lock()
        .map_err(|_| PortError::Unavailable("account changes are unavailable".into()))?;
    let path = settings_path(accounts);
    let mut settings = read_json(&path)
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    if !settings["permissions"].is_object() {
        settings["permissions"] = json!({});
    }
    if let Some(allow) = allow {
        settings["permissions"]["allow"] = json!(allow);
    }
    if let Some(deny) = deny {
        settings["permissions"]["deny"] = json!(deny);
    }
    write_json_atomic(&path, &settings)?;
    Ok(settings["permissions"].clone())
}

/// Adds one allow rule shared by every account and session; no duplicates or empty rules.
pub fn add_allow(accounts: &FileAccounts, rule: &str) -> Result<(), PortError> {
    if rule.is_empty() {
        return Ok(());
    }
    let mut allow = read(accounts).allow;
    if allow.iter().any(|r| r == rule) {
        return Ok(());
    }
    allow.push(rule.into());
    save(accounts, Some(allow), None).map(|_| ())
}

/// Keeps antiquota's hook entry current for this executable, so tool calls and mid-turn
/// messages reach the running engine. A user's choice to switch the hook off is kept.
pub fn install_hook(accounts: &FileAccounts, program: &Path) -> Result<bool, PortError> {
    let _guard = accounts
        .changes
        .lock()
        .map_err(|_| PortError::Unavailable("account changes are unavailable".into()))?;
    let path = hooks_path(accounts);
    let mut hooks = read_json(&path).unwrap_or_else(|| json!({}));
    let program = program.to_string_lossy();
    if !install_hook_entry(&mut hooks, &program, cfg!(windows)) {
        return Ok(false);
    }
    write_json_atomic(&path, &hooks)?;
    Ok(true)
}
