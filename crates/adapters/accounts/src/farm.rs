//! Owns the configuration clone, per-slot link farm and conversation store link.
use crate::{FileAccounts, Slot, check_name};
use antiquota_core::PortError;
use antiquota_store::{io_error, write_atomic};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};
const FARMS: [&str; 3] = [".gemini", ".gemini/antigravity-cli", ".gemini/antigravity"];
const SHARED: [&str; 4] = ["conversations", "brain", "annotations", "implicit"];
pub(crate) fn private(name: &str) -> bool {
    name.starts_with("antigravity-oauth-token")
        || name.ends_with("_state.pbtxt")
        || [
            "installation_id",
            "log",
            "cli.log",
            "crashes",
            "cache",
            "presence",
            "updater",
            "last_check.timestamp",
        ]
        .contains(&name)
}
pub(crate) fn clone_skip(relative: &str, name: &str) -> bool {
    (relative.is_empty() && name == "antigravity")
        || (relative == "config" && name == "projects")
        || (relative == "antigravity-cli"
            && [
                "conversations",
                "brain",
                "annotations",
                "implicit",
                "history.jsonl",
                "bin",
                "builtin",
                "scratch",
            ]
            .contains(&name))
        || ((relative.is_empty() || relative == "antigravity-cli")
            && (private(name)
                || name.starts_with("conversation_summaries.db")
                || name.ends_with("_summaries_proto.pb")))
}
pub fn seed(accounts: &FileAccounts, force: bool) -> Result<(), PortError> {
    let own = accounts.root.join("stack/.gemini");
    if linked(&own) || linked(&accounts.root.join("stack")) {
        return Err(PortError::Invalid("own stack root cannot be linked".into()));
    }
    if linked(&own.join("antigravity-cli")) {
        return Err(PortError::Invalid(
            "own stack CLI directory cannot be linked".into(),
        ));
    }
    if force || !own.is_dir() {
        if let Some(value) =
            crate::files::read_json(&accounts.home.join(".gemini/antigravity-cli/settings.json"))
        {
            crate::stack_edit::validate_permissions(&value)?;
        }
        let protected = crate::files::read_json(&own.join("config/hooks.json"))
            .and_then(|value| value.get("antiquota").cloned());
        fs::create_dir_all(&own).map_err(io_error)?;
        let source = accounts.home.join(".gemini");
        if source.is_dir() {
            for entry in fs::read_dir(&source).map_err(io_error)? {
                let entry = entry.map_err(io_error)?;
                let name = entry.file_name().to_string_lossy().into_owned();
                if clone_skip("", &name) {
                    continue;
                }
                if name == "antigravity-cli" && entry.path().is_dir() {
                    fs::create_dir_all(own.join(&name)).map_err(io_error)?;
                    for child in fs::read_dir(entry.path()).map_err(io_error)? {
                        let child = child.map_err(io_error)?;
                        let name = child.file_name().to_string_lossy().into_owned();
                        if !clone_skip("antigravity-cli", &name) {
                            replace(
                                &child.path(),
                                &own.join("antigravity-cli").join(&name),
                                &format!("antigravity-cli/{name}"),
                                &mut BTreeSet::new(),
                            )?;
                        }
                    }
                } else {
                    replace(&entry.path(), &own.join(&name), &name, &mut BTreeSet::new())?;
                }
            }
        }
        for relative in [
            "config/hooks.json",
            "config/mcp_config.json",
            "settings.json",
        ] {
            let path = own.join(relative);
            if let Ok(text) = fs::read_to_string(&path) {
                write_atomic(&path, rewrite_paths(accounts, &text).as_bytes())?;
            }
        }
        if let Some(protected) = protected {
            let path = own.join("config/hooks.json");
            let mut hooks = crate::files::read_json(&path)
                .filter(serde_json::Value::is_object)
                .unwrap_or(serde_json::json!({}));
            hooks["antiquota"] = protected;
            antiquota_store::write_json_atomic(&path, &hooks)?;
        }
    }
    for name in SHARED {
        let dir = own.join("antigravity-cli").join(name);
        if linked(&dir) {
            return Err(PortError::Invalid(
                "shared conversation directory cannot be linked outside the stack".into(),
            ));
        }
        fs::create_dir_all(dir).map_err(io_error)?;
    }
    expose_usage(accounts)?;
    Ok(())
}
pub(crate) fn replace(
    source: &Path,
    target: &Path,
    relative: &str,
    seen: &mut BTreeSet<PathBuf>,
) -> Result<(), PortError> {
    let Ok(canonical) = source.canonicalize() else {
        return Ok(());
    };
    if !seen.insert(canonical.clone()) {
        return Err(PortError::Invalid(
            "configuration contains a symbolic link cycle".into(),
        ));
    }
    remove_entry(target)?;
    if canonical.is_dir() {
        fs::create_dir_all(target).map_err(io_error)?;
        for entry in fs::read_dir(&canonical).map_err(io_error)? {
            let entry = entry.map_err(io_error)?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if !clone_skip(relative, &name) {
                replace(
                    &entry.path(),
                    &target.join(&name),
                    &format!("{relative}/{name}"),
                    seen,
                )?;
            }
        }
    } else {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(io_error)?;
        }
        fs::copy(&canonical, target).map_err(io_error)?;
    }
    seen.remove(&canonical);
    Ok(())
}
pub fn rewrite_paths(accounts: &FileAccounts, text: &str) -> String {
    let source = accounts.home.join(".gemini").to_string_lossy().into_owned();
    let own = accounts
        .root
        .join("stack/.gemini")
        .to_string_lossy()
        .into_owned();
    let mut result = text.to_owned();
    for sub in ["config/", "antigravity-cli/", "GEMINI.md", "settings.json"] {
        result = result.replace(&format!("{source}/{sub}"), &format!("{own}/{sub}"));
    }
    result
}
pub fn sync(accounts: &FileAccounts, slot: &Slot) -> Result<(), PortError> {
    check_name(&slot.name)?;
    let expected = accounts.root.join("slots").join(&slot.name);
    if Path::new(&slot.home) != expected || linked(&expected) {
        return Err(PortError::Invalid(
            "account HOME is outside its slot".into(),
        ));
    }
    let _change = accounts
        .changes
        .lock()
        .map_err(|_| PortError::Unavailable("account changes unavailable".into()))?;
    seed(accounts, false)?;
    farm(accounts, &expected, "")
}
fn farm(accounts: &FileAccounts, slot: &Path, relative: &str) -> Result<(), PortError> {
    let source = if relative.starts_with(".gemini") {
        accounts.root.join("stack").join(relative)
    } else {
        accounts.home.join(relative)
    };
    let target = slot.join(relative);
    if linked(&target) {
        remove_entry(&target)?;
    }
    fs::create_dir_all(&target).map_err(io_error)?;
    let mut entries = vec![];
    if source.is_dir() {
        for entry in fs::read_dir(&source).map_err(io_error)? {
            entries.push(entry.map_err(io_error)?.file_name());
        }
    }
    if relative.is_empty() {
        entries.retain(|n| n != ".gemini");
        entries.push(".gemini".into());
    }
    let root = accounts.root.canonicalize().ok();
    for name in entries {
        let name = name.to_string_lossy();
        let sub = if relative.is_empty() {
            name.to_string()
        } else {
            format!("{relative}/{name}")
        };
        let src = if sub.starts_with(".gemini") {
            accounts.root.join("stack").join(&sub)
        } else {
            accounts.home.join(&sub)
        };
        let dst = slot.join(&sub);
        if FARMS.contains(&sub.as_str()) && src.is_dir() {
            farm(accounts, slot, &sub)?;
        } else if !(FARMS.contains(&relative) && private(&name))
            && src.canonicalize().ok() != root
            && fs::symlink_metadata(&dst).is_err()
            && src.exists()
        {
            link(&src, &dst)?;
        }
    }
    for entry in fs::read_dir(target).map_err(io_error)? {
        let p = entry.map_err(io_error)?.path();
        if linked(&p) && !p.exists() {
            remove_entry(&p)?;
        }
    }
    Ok(())
}
pub fn expose_usage(accounts: &FileAccounts) -> Result<bool, PortError> {
    if std::env::var_os("ANTIQUOTA_NO_USAGE_LINK").is_some() {
        return Ok(false);
    }
    let path = accounts.home.join(".config/antigravity");
    let target = accounts.root.join("stack/.gemini/antigravity-cli");
    if linked(&path) {
        if path.canonicalize().ok() == target.canonicalize().ok() {
            return Ok(true);
        }
        if path.exists() {
            return Ok(false);
        }
        remove_entry(&path)?;
    } else if path.exists() {
        return Ok(false);
    }
    fs::create_dir_all(path.parent().unwrap_or(Path::new("."))).map_err(io_error)?;
    link(&target, &path)?;
    Ok(true)
}
pub(crate) fn linked(path: &Path) -> bool {
    #[cfg(windows)]
    if junction::exists(path).unwrap_or(false) {
        return true;
    }
    path.is_symlink()
}
pub(crate) fn remove_entry(path: &Path) -> Result<(), PortError> {
    // A junction must be detached before removing a directory tree.
    // https://docs.rs/junction/2.1.0/junction/fn.delete.html
    // A plain file or folder is not a reparse point, which `exists` reports as an error.
    #[cfg(windows)]
    if junction::exists(path).unwrap_or(false) {
        junction::delete(path).map_err(io_error)?;
        return Ok(());
    }

    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => {
            fs::remove_file(path).map_err(io_error)
        }
        Ok(_) => fs::remove_dir_all(path).map_err(io_error),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(io_error(e)),
    }
}
pub fn link(source: &Path, target: &Path) -> Result<(), PortError> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(source, target).map_err(io_error)
    }
    #[cfg(windows)]
    {
        if source.is_dir() {
            std::os::windows::fs::symlink_dir(source, target)
                .or_else(|_| junction::create(source, target))
                .map_err(io_error)
        } else {
            std::os::windows::fs::symlink_file(source, target)
                .or_else(|_| fs::hard_link(source, target))
                .or_else(|_| fs::copy(source, target).map(|_| ()))
                .map_err(io_error)
        }
    }
}
