//! Owns compatible quota caches, walls, verification markers and routing history.
use crate::{FileAccounts, check_name};
use antiquota_core::{AccountStore, PortError, Quota, RoutingEntry};
use antiquota_store::{append_json, io_error, read_values, write_atomic, write_json_atomic};
use serde_json::{Value, json};
use std::{fs, path::Path};
pub fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}
pub fn read_json(path: &Path) -> Option<Value> {
    fs::read(path)
        .ok()
        .and_then(|v| serde_json::from_slice(&v).ok())
}
fn path(
    accounts: &FileAccounts,
    kind: &str,
    name: &str,
    suffix: &str,
) -> Result<std::path::PathBuf, PortError> {
    check_name(name)?;
    let dir = accounts.root.join("state").join(kind);
    if dir.is_symlink() {
        return Err(PortError::Invalid(
            "private account state directory cannot be linked".into(),
        ));
    }
    Ok(dir.join(format!("{name}{suffix}")))
}
pub fn pause(accounts: &FileAccounts, name: &str, on: bool) -> Result<(), PortError> {
    accounts.slot(name)?;
    let p = path(accounts, "paused", name, "")?;
    if on {
        write_atomic(&p, b"")
    } else {
        unlink(&p)
    }
}
pub fn remove(accounts: &FileAccounts, name: &str) -> Result<(), PortError> {
    let slot = accounts.slot(name)?;
    let Some(_lease) = accounts.lease(slot.clone())? else {
        return Err(PortError::Invalid(format!(
            "'{name}' is running a call; stop it first"
        )));
    };
    let _change = accounts
        .changes
        .lock()
        .map_err(|_| PortError::Unavailable("account changes unavailable".into()))?;
    fs::remove_dir_all(slot.home).map_err(io_error)?;
    for bucket in ["gemini", "cg"] {
        clear_wall(accounts, name, bucket)?;
    }
    Ok(())
}
pub fn save_quota(
    accounts: &FileAccounts,
    name: &str,
    quota: &Quota,
    now: f64,
) -> Result<(), PortError> {
    let _change = accounts
        .changes
        .lock()
        .map_err(|_| PortError::Unavailable("account changes unavailable".into()))?;
    let p = path(accounts, "quota", name, ".json")?;
    let mut old = read_json(&p).filter(Value::is_object).unwrap_or(json!({}));
    let mut pools = old
        .get("quota")
        .cloned()
        .filter(Value::is_object)
        .unwrap_or(json!({}));
    for (bucket, windows) in &quota.0 {
        let mut previous = pools
            .get(bucket)
            .cloned()
            .filter(Value::is_object)
            .unwrap_or(json!({}));
        for (window, value) in windows {
            previous[window] = json!(value);
        }
        pools[bucket] = previous;
    }
    old["at"] = json!(now);
    old["quota"] = pools;
    write_json_atomic(&p, &old)
}
pub fn set_wall(
    accounts: &FileAccounts,
    name: &str,
    bucket: &str,
    until: f64,
    why: &str,
) -> Result<(), PortError> {
    if !["gemini", "cg"].contains(&bucket) || !until.is_finite() || until < 0.0 {
        return Err(PortError::Invalid("invalid account wall".into()));
    }
    write_atomic(
        &path(accounts, "walls", name, &format!(".{bucket}"))?,
        format!("{} {}\n", until.floor(), why.replace(['\n', '\r'], " ")).as_bytes(),
    )
}
pub fn clear_wall(accounts: &FileAccounts, name: &str, bucket: &str) -> Result<(), PortError> {
    if !["gemini", "cg"].contains(&bucket) {
        return Err(PortError::Invalid("invalid quota bucket".into()));
    }
    unlink(&path(accounts, "walls", name, &format!(".{bucket}"))?)
}
pub fn wall(accounts: &FileAccounts, name: &str, bucket: &str, now: f64) -> (f64, String) {
    let Ok(p) = path(accounts, "walls", name, &format!(".{bucket}")) else {
        return (0.0, String::new());
    };
    let text = fs::read_to_string(p).unwrap_or_default();
    let (stamp, why) = text.trim().split_once(' ').unwrap_or((text.trim(), ""));
    match stamp.parse::<f64>() {
        Ok(value) if value.is_finite() && value > now => (value, why.into()),
        _ => (0.0, String::new()),
    }
}
pub fn set_verify(
    accounts: &FileAccounts,
    name: &str,
    url: Option<&str>,
    now: f64,
) -> Result<(), PortError> {
    let p = path(accounts, "verify", name, ".json")?;
    if let Some(url) = url {
        let mut old = read_json(&p).filter(Value::is_object).unwrap_or(json!({}));
        old["url"] = json!(url);
        old["since"] = json!(now);
        write_json_atomic(&p, &old)
    } else {
        unlink(&p)
    }
}
pub fn routes(accounts: &FileAccounts, n: usize) -> Result<Vec<RoutingEntry>, PortError> {
    let rows = read_values(&accounts.root.join("state/routing.jsonl"))?;
    let start = rows.len().saturating_sub(n);
    Ok(rows
        .into_iter()
        .skip(start)
        .filter_map(|v| serde_json::from_str(&v.to_string()).ok())
        .collect())
}
pub fn route(accounts: &FileAccounts, entry: &RoutingEntry) -> Result<(), PortError> {
    let _writer = accounts
        .changes
        .lock()
        .map_err(|_| PortError::Unavailable("routing writer unavailable".into()))?;
    let p = accounts.root.join("state/routing.jsonl");
    fs::create_dir_all(p.parent().unwrap_or(Path::new("."))).map_err(io_error)?;
    append_json(&p, entry)?;
    if fs::metadata(&p).map_err(io_error)?.len() > 2_000_000 {
        let text = fs::read_to_string(&p).map_err(io_error)?;
        let lines: Vec<_> = text.split_inclusive('\n').collect();
        write_atomic(
            &p,
            lines[lines.len().saturating_sub(5000)..]
                .concat()
                .as_bytes(),
        )?;
    }
    Ok(())
}
pub fn unlink(path: &Path) -> Result<(), PortError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(io_error(e)),
    }
}
