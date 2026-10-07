//! Owns bounded file snapshots and legacy unified diffs for edit-tool events.
use antiquota_core::Event;
use similar::TextDiff;
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};
pub struct FileDiff {
    cwd: PathBuf,
    started: SystemTime,
    before: BTreeMap<u64, Option<String>>,
    remembered: BTreeMap<PathBuf, Option<String>>,
}
impl FileDiff {
    pub fn new(cwd: PathBuf) -> Self {
        Self {
            cwd,
            started: SystemTime::now(),
            before: BTreeMap::new(),
            remembered: BTreeMap::new(),
        }
    }
    pub fn decorate(&mut self, event: &mut Event) {
        let Event::Tool {
            step,
            name,
            params,
            state,
            diff,
            ..
        } = event
        else {
            return;
        };
        if name == "view_file" {
            if let Some(path) = params["AbsolutePath"].as_str() {
                let path = self.path(path);
                self.remembered.insert(path.clone(), read(&path));
            }
            return;
        }
        if !matches!(
            name.as_str(),
            "write_to_file" | "replace_file_content" | "multi_replace_file_content" | "sed_file"
        ) {
            return;
        }
        let Some(path) = params["TargetFile"].as_str() else {
            return;
        };
        let path = self.path(path);
        if state == "running" {
            self.before.entry(*step).or_insert_with(|| read(&path));
            return;
        }
        let after = read(&path);
        let mut before = self.before.get(step).cloned().flatten();
        if (before.is_none() || before == after) && self.remembered.contains_key(&path) {
            before = self.remembered.get(&path).cloned().flatten();
        } else if before == after && name == "write_to_file" && born_after(&path, self.started) {
            before = Some(String::new());
        }
        *diff = Some(unified(
            &path,
            before.as_deref(),
            after.as_deref(),
            &self.cwd,
        ));
        self.remembered.insert(path, after);
    }
    fn path(&self, path: &str) -> PathBuf {
        let path = PathBuf::from(path);
        if path.is_absolute() {
            path
        } else {
            self.cwd.join(path)
        }
    }
}
fn read(path: &Path) -> Option<String> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Some(String::new()),
        Err(_) => return None,
    };
    if file.metadata().ok()?.len() > 1_000_000 {
        return None;
    }
    let mut bytes = vec![];
    file.take(1_000_001).read_to_end(&mut bytes).ok()?;
    if bytes.len() > 1_000_000 {
        return None;
    }
    String::from_utf8(bytes).ok()
}
fn born_after(path: &Path, started: SystemTime) -> bool {
    fs::metadata(path)
        .ok()
        .and_then(|m| m.created().or_else(|_| m.modified()).ok())
        .is_some_and(|created| {
            created
                >= started
                    .checked_sub(Duration::from_secs(1))
                    .unwrap_or(started)
        })
}
fn unified(path: &Path, before: Option<&str>, after: Option<&str>, cwd: &Path) -> String {
    let (Some(before), Some(after)) = (before, after) else {
        return String::new();
    };
    if before == after {
        return String::new();
    }
    let relative = path.strip_prefix(cwd).unwrap_or(path).to_string_lossy();
    let diff = TextDiff::from_lines(before, after);
    let mut output = vec![format!("--- a/{relative}\n"), format!("+++ b/{relative}\n")];
    for hunk in diff.unified_diff().context_radius(3).iter_hunks() {
        output.push(format!("{}\n", hunk.header()));
        for change in hunk.iter_changes() {
            output.push(format!("{}{}", change.tag(), change.value()));
            if output.len() >= 600 {
                return output.concat();
            }
        }
    }
    output.concat()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn new_file_append_and_no_newline_match_legacy_diff() {
        let cwd = Path::new("/project");
        let path = cwd.join("notes.txt");
        assert_eq!(
            unified(&path, Some(""), Some("turn 1\n"), cwd),
            "--- a/notes.txt\n+++ b/notes.txt\n@@ -0,0 +1 @@\n+turn 1\n"
        );
        assert_eq!(
            unified(&path, Some("turn 1\n"), Some("turn 1\nturn 2\n"), cwd),
            "--- a/notes.txt\n+++ b/notes.txt\n@@ -1 +1,2 @@\n turn 1\n+turn 2\n"
        );
        assert_eq!(
            unified(&path, Some("old"), Some("new"), cwd),
            "--- a/notes.txt\n+++ b/notes.txt\n@@ -1 +1 @@\n-old+new"
        );
        assert!(unified(&path, None, Some("text"), cwd).is_empty());
    }
    #[test]
    fn absent_binary_large_and_line_cap_are_bounded() {
        let owner = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).unwrap();
        let root = PathBuf::from(owner)
            .join("Desktop/test/antiquota-diff")
            .join(format!(
                "{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("file");
        assert_eq!(read(&path), Some(String::new()));
        fs::write(&path, [255, 254]).unwrap();
        assert_eq!(read(&path), None);
        fs::write(&path, vec![b'x'; 1_000_001]).unwrap();
        assert_eq!(read(&path), None);
        let output = unified(&path, Some(""), Some(&"line\n".repeat(1000)), &root);
        assert_eq!(output.lines().count(), 600);
        fs::remove_dir_all(root).unwrap();
    }
}
