//! Owns bounded mention discovery, project notes and the HOME folder picker.
use crate::{io_error, user_home};
use antiquota_core::PortError;
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn mention_files(cwd: &str, query: &str) -> Result<Vec<String>, PortError> {
    fn visit(root: &Path, path: &Path, q: &str, hits: &mut Vec<String>) -> Result<(), PortError> {
        let mut entries: Vec<_> = fs::read_dir(path)
            .map_err(io_error)?
            .collect::<Result<_, _>>()
            .map_err(io_error)?;
        entries.sort_by_key(|e| e.file_name());
        let mut dirs = vec![];
        for e in entries {
            let name = e.file_name().to_string_lossy().into_owned();
            if e.file_type().map_err(io_error)?.is_dir() {
                if !name.starts_with('.')
                    && !["node_modules", "__pycache__", "venv", "dist", "build"]
                        .contains(&name.as_str())
                {
                    dirs.push(e.path());
                }
            } else if e.path().is_file()
                && crate::canonical(e.path()).is_ok_and(|p| p.starts_with(root))
            {
                let rel = e
                    .path()
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                if rel.to_lowercase().contains(q) {
                    hits.push(rel);
                }
                if hits.len() >= 400 {
                    return Ok(());
                }
            }
        }
        for dir in dirs {
            visit(root, &dir, q, hits)?;
            if hits.len() >= 400 {
                break;
            }
        }
        Ok(())
    }
    let root = crate::canonical(cwd).map_err(io_error)?;
    let q = query.to_lowercase();
    let mut hits = vec![];
    visit(&root, &root, &q, &mut hits)?;
    hits.sort_by_key(|r| {
        (
            !Path::new(r)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase()
                .starts_with(&q),
            r.chars().count(),
        )
    });
    hits.truncate(30);
    Ok(hits)
}
pub fn browse(path: &str) -> Result<Value, PortError> {
    let home = crate::canonical(user_home()?).map_err(io_error)?;
    let expanded = if path == "~" || path.is_empty() {
        home.clone()
    } else if let Some(rest) = path.strip_prefix("~/") {
        home.join(rest)
    } else {
        PathBuf::from(path)
    };
    let path = crate::canonical(expanded)
        .ok()
        .filter(|p| p.starts_with(&home) && p.is_dir())
        .unwrap_or(home.clone());
    let mut dirs: Vec<_> = fs::read_dir(&path)
        .map_err(io_error)?
        .filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            (!name.starts_with('.')
                && e.path().is_dir()
                && crate::canonical(e.path()).is_ok_and(|p| p.starts_with(&home)))
            .then_some(name)
        })
        .collect();
    dirs.sort_by_key(|s| s.to_lowercase());
    Ok(
        json!({"path":path.to_string_lossy(),"parent":if path==home {None}else{path.parent().map(|p|p.to_string_lossy().into_owned())},"dirs":dirs}),
    )
}
pub fn note(cwd: &str, text: &str) -> Result<Value, PortError> {
    let root = crate::canonical(cwd).map_err(io_error)?;
    let path = ["GEMINI.md", "AGENTS.md"]
        .into_iter()
        .map(|n| root.join(n))
        .find(|p| p.is_file())
        .unwrap_or(root.join("GEMINI.md"));
    if path.is_symlink() {
        return Err(PortError::Invalid("project note cannot be linked".into()));
    }
    use std::io::{Read, Seek, SeekFrom, Write};
    static NOTES: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = NOTES
        .lock()
        .map_err(|_| PortError::Unavailable("project notes unavailable".into()))?;
    let mut file = fs::OpenOptions::new()
        .read(true)
        .append(true)
        .create(true)
        .open(&path)
        .map_err(io_error)?;
    let mut prefix = "";
    if file.metadata().map_err(io_error)?.len() > 0 {
        file.seek(SeekFrom::End(-1)).map_err(io_error)?;
        let mut last = [0];
        file.read_exact(&mut last).map_err(io_error)?;
        if last[0] != b'\n' {
            prefix = "\n";
        }
    }
    file.write_all(format!("{prefix}- {}\n", text.trim()).as_bytes())
        .map_err(io_error)?;
    file.sync_all().map_err(io_error)?;
    Ok(json!({"path":path.to_string_lossy(),"file":path.file_name().unwrap().to_string_lossy()}))
}
