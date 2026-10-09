//! Stores first-before/latest-after file snapshots and refuses rewind over later user edits.
use crate::{FileStore, io_error, write_atomic, write_json_atomic};
use antiquota_core::PortError;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

fn contents(path: &Path) -> Result<(String, bool), PortError> {
    use std::io::Read;
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((String::new(), false)),
        Err(e) => return Err(io_error(e)),
    };
    let mut bytes = vec![];
    file.take(1_000_001)
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    if bytes.len() > 1_000_000 || bytes.iter().take(8000).any(|b| *b == 0) {
        return Err(PortError::Invalid(
            "file is not a readable checkpoint snapshot".into(),
        ));
    }
    Ok((
        String::from_utf8(bytes)
            .map_err(|_| PortError::Invalid("checkpoint is not UTF-8 text".into()))?,
        true,
    ))
}
fn target(cwd: &str, path: &str) -> Result<PathBuf, PortError> {
    let root = crate::canonical(cwd).map_err(io_error)?;
    let lexical = root.join(path);
    if !lexical.starts_with(&root)
        || lexical
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(PortError::Invalid(
            "checkpoint must stay inside the project".into(),
        ));
    }
    let mut ancestor = lexical
        .parent()
        .ok_or_else(|| PortError::Invalid("invalid checkpoint path".into()))?;
    while !ancestor.exists() {
        ancestor = ancestor
            .parent()
            .ok_or_else(|| PortError::Invalid("invalid checkpoint path".into()))?;
    }
    let canonical = crate::canonical(ancestor).map_err(io_error)?;
    if !canonical.starts_with(&root) {
        return Err(PortError::Invalid(
            "checkpoint must stay inside the project".into(),
        ));
    }
    let target = canonical.join(
        lexical
            .strip_prefix(ancestor)
            .map_err(|e| PortError::Invalid(e.to_string()))?,
    );
    if target.is_symlink() {
        return Err(PortError::Invalid(
            "checkpoint file cannot be linked".into(),
        ));
    }
    Ok(target)
}
impl FileStore {
    pub(crate) fn capture_checkpoint(
        &self,
        sid: &str,
        turn: u64,
        cwd: &str,
        path: &str,
        before: bool,
    ) -> Result<(), PortError> {
        // Tool calls outside the project are not rewindable here.
        let Ok(target) = target(cwd, path) else {
            return Ok(());
        };
        let Ok((text, exists)) = contents(&target) else {
            return Ok(());
        };
        let file = self
            .session_dir(sid)?
            .join(format!("checkpoints/{turn}.json"));
        let mut map: Value = match fs::read(&file) {
            Ok(b) => serde_json::from_slice(&b).map_err(|e| PortError::Invalid(e.to_string()))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => json!({}),
            Err(e) => return Err(io_error(e)),
        };
        let path = target.to_string_lossy().into_owned();
        if before {
            if map.get(&path).is_none() {
                map[&path] = json!({"before":text,"existed":exists});
            }
        } else if map.get(&path).is_some() {
            map[&path]["after"] = json!(text);
            map[&path]["after_exists"] = json!(exists);
        } else {
            return Ok(());
        }
        write_json_atomic(&file, &map)
    }
    pub(crate) fn restore_checkpoints(
        &self,
        sid: &str,
        turn: u64,
        cwd: &str,
    ) -> Result<(Vec<String>, Vec<String>), PortError> {
        let dir = self.session_dir(sid)?.join("checkpoints");
        let mut turns: Vec<_> = if dir.is_dir() {
            fs::read_dir(&dir)
                .map_err(io_error)?
                .filter_map(Result::ok)
                .filter_map(|e| {
                    e.file_name()
                        .to_str()?
                        .strip_suffix(".json")?
                        .parse::<u64>()
                        .ok()
                })
                .filter(|t| *t >= turn)
                .collect()
        } else {
            vec![]
        };
        turns.sort_unstable();
        let mut restore: BTreeMap<PathBuf, Value> = BTreeMap::new();
        let mut latest: BTreeMap<PathBuf, Value> = BTreeMap::new();
        for t in &turns {
            let bytes = fs::read(dir.join(format!("{t}.json"))).map_err(io_error)?;
            let map: BTreeMap<String, Value> =
                serde_json::from_slice(&bytes).map_err(|e| PortError::Invalid(e.to_string()))?;
            for (path, entry) in map {
                if entry["after"].as_str().is_none() {
                    continue;
                }
                let path = target(cwd, &path)?;
                if entry["before"].as_str().is_none() {
                    return Err(PortError::Invalid("invalid checkpoint contents".into()));
                }
                if entry["before"] == "" && entry["existed"].as_bool().is_none() {
                    return Err(PortError::Invalid("legacy checkpoint cannot distinguish an empty file from a new file; rewind refused".into()));
                }
                restore.entry(path.clone()).or_insert(entry.clone());
                latest.insert(path, entry);
            }
        }
        if restore.is_empty() {
            return Err(PortError::Invalid(format!(
                "no file changes from turn {turn} on"
            )));
        }
        let root = crate::canonical(cwd).map_err(io_error)?;
        let relative = |path: &Path| {
            path.strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .into_owned()
        };
        let mut outside = vec![];
        for (path, entry) in &latest {
            let (text, exists) = contents(path)?;
            if text != entry["after"].as_str().unwrap()
                || entry["after_exists"].as_bool().is_some_and(|e| e != exists)
            {
                outside.push(relative(path));
            }
        }
        if !outside.is_empty() {
            return Err(PortError::Invalid(format!(
                "files changed after the agent's last edit; rewind refused: {}",
                outside.join(", ")
            )));
        }
        for (path, entry) in &restore {
            let before = entry["before"].as_str().unwrap();
            let existed = entry["existed"].as_bool().unwrap_or(!before.is_empty());
            if existed {
                write_atomic(path, before.as_bytes())?;
            } else if path.exists() {
                fs::remove_file(path).map_err(io_error)?;
            }
        }
        for t in turns {
            fs::remove_file(dir.join(format!("{t}.json"))).map_err(io_error)?;
        }
        Ok((restore.keys().map(|p| relative(p)).collect(), outside))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checkpoints_keep_empty_files_refuse_user_edits_and_preserve_ambiguous_legacy_data() {
        let root = PathBuf::from(std::env::var_os("HOME").unwrap())
            .join("Desktop/test")
            .join(format!("checkpoint-{}", uuid::Uuid::new_v4()));
        let project = root.join("project");
        fs::create_dir_all(&project).unwrap();
        let store = FileStore::open(root.join("data")).unwrap();
        let cwd = project.to_str().unwrap();
        let file = project.join("empty.txt");
        fs::write(&file, "").unwrap();
        store
            .capture_checkpoint("test", 1, cwd, "empty.txt", true)
            .unwrap();
        fs::write(&file, "one").unwrap();
        store
            .capture_checkpoint("test", 1, cwd, "empty.txt", false)
            .unwrap();
        store
            .capture_checkpoint("test", 1, cwd, "empty.txt", true)
            .unwrap();
        fs::write(&file, "two").unwrap();
        store
            .capture_checkpoint("test", 1, cwd, "empty.txt", false)
            .unwrap();
        store
            .capture_checkpoint("test", 2, cwd, "empty.txt", true)
            .unwrap();
        fs::write(&file, "three").unwrap();
        store
            .capture_checkpoint("test", 2, cwd, "empty.txt", false)
            .unwrap();
        let checkpoint = store
            .session_dir("test")
            .unwrap()
            .join("checkpoints/1.json");
        let map: Value = serde_json::from_slice(&fs::read(&checkpoint).unwrap()).unwrap();
        // Keys are canonical paths: on Windows the full name, without the `\\?\` prefix.
        let key = crate::canonical(&file)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert_eq!(map[&key]["before"], "");
        assert_eq!(map[&key]["after"], "two");
        fs::write(&file, "later user edit").unwrap();
        assert!(
            store
                .restore_checkpoints("test", 1, cwd)
                .unwrap_err()
                .to_string()
                .contains("rewind refused")
        );
        assert_eq!(fs::read_to_string(&file).unwrap(), "later user edit");
        assert!(checkpoint.exists());
        fs::write(&file, "three").unwrap();
        assert_eq!(
            store.restore_checkpoints("test", 1, cwd).unwrap().0,
            vec!["empty.txt"]
        );
        assert!(file.exists());
        assert_eq!(fs::read_to_string(&file).unwrap(), "");
        assert!(!checkpoint.exists());
        store
            .capture_checkpoint("test", 3, cwd, "new/nested.txt", true)
            .unwrap();
        fs::create_dir_all(project.join("new")).unwrap();
        fs::write(project.join("new/nested.txt"), "created").unwrap();
        store
            .capture_checkpoint("test", 3, cwd, "new/nested.txt", false)
            .unwrap();
        store.restore_checkpoints("test", 3, cwd).unwrap();
        assert!(!project.join("new/nested.txt").exists());
        fs::write(&file, "agent edit").unwrap();
        write_json_atomic(
            &checkpoint,
            &json!({key.as_str():{"before":"","after":"agent edit"}}),
        )
        .unwrap();
        assert!(
            store
                .restore_checkpoints("test", 1, cwd)
                .unwrap_err()
                .to_string()
                .contains("legacy checkpoint")
        );
        assert!(checkpoint.exists());
        assert_eq!(fs::read_to_string(&file).unwrap(), "agent edit");
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn rewind_refuses_link_escape_before_changing_any_file() {
        let root = PathBuf::from(std::env::var_os("HOME").unwrap())
            .join("Desktop/test")
            .join(format!("checkpoint-link-{}", uuid::Uuid::new_v4()));
        let project = root.join("project");
        fs::create_dir_all(&project).unwrap();
        let outside = root.join("outside.txt");
        fs::write(&outside, "private").unwrap();
        std::os::unix::fs::symlink(&outside, project.join("link")).unwrap();
        let store = FileStore::open(root.join("data")).unwrap();
        let cp = store
            .session_dir("test")
            .unwrap()
            .join("checkpoints/1.json");
        write_json_atomic(&cp,&json!({project.join("link").to_str().unwrap():{"before":"before","after":"private","existed":true}})).unwrap();
        assert!(
            store
                .restore_checkpoints("test", 1, project.to_str().unwrap())
                .is_err()
        );
        assert_eq!(fs::read_to_string(&outside).unwrap(), "private");
        assert!(cp.exists());
        fs::remove_dir_all(root).unwrap();
    }
}
