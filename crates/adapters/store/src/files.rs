//! Owns atomic files, append boundaries, discovery data, tokens, and OS locks.
use crate::io_error;
use antiquota_core::PortError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineInfo {
    pub pid: u32,
    pub port: u16,
    pub url: String,
    pub version: String,
    pub started: f64,
    pub by: String,
}
pub struct FileLease {
    _file: File,
}
pub fn lock_file(path: &Path) -> Result<Option<FileLease>, PortError> {
    lock(path, false)
}
/// A lock that other shared holders may hold at the same time; an exclusive lock excludes it.
pub fn lock_file_shared(path: &Path) -> Result<Option<FileLease>, PortError> {
    lock(path, true)
}
fn lock(path: &Path, shared: bool) -> Result<Option<FileLease>, PortError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(io_error)?;
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(path).map_err(io_error)?;
    let locked = if shared {
        file.try_lock_shared()
    } else {
        file.try_lock()
    };
    match locked {
        Ok(()) => Ok(Some(FileLease { _file: file })),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(e)) => Err(io_error(e)),
    }
}
pub fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<(), PortError> {
    let bytes = serde_json::to_vec(value).map_err(|e| PortError::Invalid(e.to_string()))?;
    write_atomic(path, &bytes)
}
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), PortError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(io_error)?;
    }
    if path.is_symlink() {
        return Err(PortError::Invalid(
            "refusing to replace a linked private file".into(),
        ));
    }
    let temporary = path.with_file_name(format!(
        ".{}.{}",
        path.file_name().unwrap_or_default().to_string_lossy(),
        uuid::Uuid::new_v4()
    ));
    let result = (|| {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary).map_err(io_error)?;
        if let Ok(old) = fs::metadata(path) {
            fs::set_permissions(&temporary, old.permissions()).map_err(io_error)?;
        }
        file.write_all(bytes).map_err(io_error)?;
        file.sync_all().map_err(io_error)?;
        drop(file);
        fs::rename(&temporary, path).map_err(io_error)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}
pub fn append_json<T: Serialize>(path: &Path, value: &T) -> Result<(), PortError> {
    if path.is_symlink() {
        return Err(PortError::Invalid(
            "refusing to append through a symbolic link".into(),
        ));
    }
    let unterminated = match File::open(path) {
        Ok(mut old) => {
            if old.metadata().map_err(io_error)?.len() == 0 {
                false
            } else {
                old.seek(SeekFrom::End(-1)).map_err(io_error)?;
                let mut last = [0];
                old.read_exact(&mut last).map_err(io_error)?;
                last[0] != b'\n'
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(e) => return Err(io_error(e)),
    };
    let mut options = OpenOptions::new();
    options.append(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(io_error)?;
    if unterminated {
        tracing::warn!(path=%path.display(),"unterminated JSONL line retained; appending a safe boundary");
        file.write_all(b"\n").map_err(io_error)?;
    }
    serde_json::to_writer(&mut file, value).map_err(|e| PortError::Unavailable(e.to_string()))?;
    file.write_all(b"\n").map_err(io_error)?;
    file.sync_all().map_err(io_error)
}
pub fn read_values(path: &Path) -> Result<Vec<Value>, PortError> {
    let text = match fs::read_to_string(path) {
        Ok(x) => x,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(io_error(e)),
    };
    Ok(text
        .lines()
        .filter_map(|line| {
            if line.trim().is_empty() {
                return None;
            }
            match serde_json::from_str(line) {
                Ok(row) => Some(row),
                Err(e) => {
                    tracing::warn!(path=%path.display(),error=%e,"malformed ledger line retained");
                    None
                }
            }
        })
        .collect())
}
pub fn token(root: &Path) -> Result<String, PortError> {
    let path = root.join("state/gui-token");
    if path.exists() {
        if path.is_symlink() {
            return Err(PortError::Invalid("GUI token cannot be linked".into()));
        }
        let text = fs::read_to_string(&path).map_err(io_error)?;
        let text = text.trim();
        if text.len() >= 32 {
            return Ok(text.into());
        }
    }
    let value = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    write_atomic(&path, value.as_bytes())?;
    Ok(value)
}
pub fn data_root() -> Result<PathBuf, PortError> {
    if let Some(dir) = std::env::var_os("ANTIQUOTA_DIR") {
        return Ok(PathBuf::from(dir));
    }
    let home = user_home()?;
    let root = home.join(".antiquota");
    static MIGRATED: std::sync::Once = std::sync::Once::new();
    MIGRATED.call_once(|| {
        if let Err(e) = migrate_from_agyloop(&home.join(".agyloop"), &root, &home) {
            tracing::warn!(error = %e, "could not finish moving ~/.agyloop");
        }
    });
    Ok(root)
}
pub fn user_home() -> Result<PathBuf, PortError> {
    if let Some(home) = std::env::var_os("ANTIQUOTA_HOME") {
        return Ok(PathBuf::from(home));
    }
    // Not $HOME on POSIX: inside an account's agy process $HOME is that account's folder.
    #[cfg(unix)]
    if let Ok(Some(user)) = nix::unistd::User::from_uid(nix::unistd::getuid()) {
        return Ok(user.dir);
    }
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from)
        .ok_or_else(|| PortError::Unavailable("user home is unavailable".into()))
}
/// antiquota was called agy-loop and kept everything in ~/.agyloop. Moves it once, points links
/// and own-stack config paths that name the old folder at the new one, and leaves a link behind
/// so paths recorded in old transcripts still open.
pub fn migrate_from_agyloop(old: &Path, new: &Path, home: &Path) -> Result<(), PortError> {
    if fs::symlink_metadata(new).is_ok() || old.is_symlink() || !old.is_dir() {
        return Ok(());
    }
    fs::rename(old, new).map_err(io_error)?;
    let _ = relink(new, old);
    let moved = |target: &Path| {
        target
            .strip_prefix(old)
            .ok()
            .filter(|rest| !rest.as_os_str().is_empty())
            .map(|rest| new.join(rest))
    };
    let mut folders = vec![new.to_path_buf()];
    while let Some(folder) = folders.pop() {
        for entry in fs::read_dir(&folder).map_err(io_error)?.flatten() {
            let path = entry.path();
            match fs::read_link(&path) {
                Ok(target) => {
                    if let Some(target) = moved(&target) {
                        fs::remove_file(&path).map_err(io_error)?;
                        relink(&target, &path)?;
                    }
                }
                Err(_) if path.is_dir() => folders.push(path),
                Err(_) => {}
            }
        }
    }
    let (from, to) = (
        format!("{}{}", old.display(), std::path::MAIN_SEPARATOR),
        format!("{}{}", new.display(), std::path::MAIN_SEPARATOR),
    );
    for rel in [
        ".gemini/config/hooks.json",
        ".gemini/config/mcp_config.json",
        ".gemini/settings.json",
        ".gemini/antigravity-cli/settings.json",
    ] {
        let path = new.join("stack").join(rel);
        if path.is_symlink() || !path.is_file() {
            continue;
        }
        let text = fs::read_to_string(&path).map_err(io_error)?;
        if text.contains(&from) {
            write_atomic(&path, text.replace(&from, &to).as_bytes())?;
        }
    }
    let usage = home.join(".config/antigravity");
    if let Some(target) = fs::read_link(&usage).ok().and_then(|t| moved(&t)) {
        fs::remove_file(&usage).map_err(io_error)?;
        relink(&target, &usage)?;
    }
    Ok(())
}
fn relink(target: &Path, link: &Path) -> Result<(), PortError> {
    #[cfg(unix)]
    return std::os::unix::fs::symlink(target, link).map_err(io_error);
    #[cfg(windows)]
    return if target.is_dir() {
        std::os::windows::fs::symlink_dir(target, link)
    } else {
        std::os::windows::fs::symlink_file(target, link)
    }
    .map_err(io_error);
}
