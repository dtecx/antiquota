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
    match file.try_lock() {
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
    std::env::var_os("ANTIQUOTA_DIR")
        .map(PathBuf::from)
        .map(Ok)
        .unwrap_or_else(|| user_home().map(|x| x.join(".antiquota")))
}
pub fn user_home() -> Result<PathBuf, PortError> {
    std::env::var_os("ANTIQUOTA_HOME")
        .or_else(|| std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }))
        .map(PathBuf::from)
        .ok_or_else(|| PortError::Unavailable("user home is unavailable".into()))
}
