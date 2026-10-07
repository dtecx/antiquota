//! Owns read-only views of a session's project folder: one directory listing and text files.
use antiquota_core::{PortError, Workspace};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const HEAVY: [&str; 9] = [
    ".git",
    "node_modules",
    "__pycache__",
    ".venv",
    "venv",
    ".mypy_cache",
    ".pytest_cache",
    "dist",
    "build",
];
const MAX_ENTRIES: usize = 400;
const MAX_TEXT: u64 = 2_000_000;

#[derive(Debug, Default, Clone)]
pub struct FileWorkspace;

fn real(path: &Path) -> Option<PathBuf> {
    std::fs::canonicalize(path).ok()
}

fn inside(path: &Path, root: &Path) -> bool {
    path == root || path.starts_with(root)
}

impl Workspace for FileWorkspace {
    fn tree(&self, cwd: &str, path: &str) -> Result<Value, PortError> {
        let root = real(Path::new(cwd))
            .ok_or_else(|| PortError::Invalid("only folders inside the session folder".into()))?;
        let target = real(&root.join(path))
            .filter(|p| inside(p, &root) && p.is_dir())
            .ok_or_else(|| PortError::Invalid("only folders inside the session folder".into()))?;
        let mut names: Vec<(bool, String)> = std::fs::read_dir(&target)
            .map_err(|e| PortError::Invalid(e.to_string()))?
            .filter_map(Result::ok)
            .map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                (target.join(&name).is_dir(), name)
            })
            .collect();
        names.sort_by(|(ad, an), (bd, bn)| {
            (!ad, an.starts_with('.'), an.to_lowercase()).cmp(&(
                !bd,
                bn.starts_with('.'),
                bn.to_lowercase(),
            ))
        });
        let entries: Vec<Value> = names
            .into_iter()
            .take(MAX_ENTRIES)
            .map(|(dir, name)| {
                let size = if dir {
                    0
                } else {
                    std::fs::metadata(target.join(&name))
                        .map(|m| m.len())
                        .unwrap_or(0)
                };
                let dim = name.starts_with('.') || HEAVY.contains(&name.as_str());
                json!({"name": name, "dir": dir, "size": size, "dim": dim})
            })
            .collect();
        let rel = target
            .strip_prefix(&root)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok(json!({"root": root.to_string_lossy(), "path": rel, "entries": entries}))
    }

    fn read_text(&self, cwd: &str, roots: &[String], path: &str) -> Result<Value, PortError> {
        let refuse =
            || PortError::Invalid("only files inside the session folder can be opened here".into());
        let base = real(Path::new(cwd)).ok_or_else(refuse)?;
        let expanded = match path.strip_prefix("~/") {
            Some(rest) => crate::user_home()?.join(rest),
            None => PathBuf::from(path),
        };
        let file = real(&base.join(expanded)).ok_or_else(refuse)?;
        let allowed = std::iter::once(base.clone())
            .chain(roots.iter().filter_map(|r| real(Path::new(r))))
            .any(|root| inside(&file, &root));
        if !allowed || !file.is_file() {
            return Err(refuse());
        }
        let name = file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let size = std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
        if size > MAX_TEXT {
            return Err(PortError::Invalid(format!("{name} is over 2 MB")));
        }
        let bytes = std::fs::read(&file).map_err(|e| PortError::Invalid(e.to_string()))?;
        if bytes.iter().take(8000).any(|b| *b == 0) {
            return Err(PortError::Invalid(format!("{name} is not a text file")));
        }
        let rel = file
            .strip_prefix(&base)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| file.to_string_lossy().into_owned());
        Ok(
            json!({"path": file.to_string_lossy(), "rel": rel, "text": String::from_utf8_lossy(&bytes)}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn listings_and_files_stay_inside_the_project() {
        let root = std::env::temp_dir().join(format!("antiquota-ws-{}", std::process::id()));
        let project = root.join("project");
        std::fs::create_dir_all(project.join("src")).unwrap();
        std::fs::create_dir_all(project.join("node_modules")).unwrap();
        std::fs::create_dir_all(project.join("Docs")).unwrap();
        std::fs::write(project.join("b.txt"), "bee").unwrap();
        std::fs::write(project.join(".env"), "x").unwrap();
        std::fs::write(project.join("a.bin"), [0u8, 1, 2]).unwrap();
        std::fs::write(root.join("secret.txt"), "no").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(root.join("secret.txt"), project.join("link.txt")).unwrap();
        let ws = FileWorkspace;
        let cwd = project.to_str().unwrap();
        let tree = ws.tree(cwd, "").unwrap();
        let names: Vec<&str> = tree["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["name"].as_str().unwrap())
            .collect();
        assert_eq!(&names[..3], ["Docs", "node_modules", "src"]);
        assert_eq!(names.last(), Some(&".env"));
        let modules = &tree["entries"][1];
        assert_eq!(
            (modules["dir"].clone(), modules["dim"].clone()),
            (json!(true), json!(true))
        );
        assert_eq!(tree["path"], "");
        assert_eq!(ws.tree(cwd, "src").unwrap()["path"], "src");
        for bad in ["..", "../..", "b.txt", "missing"] {
            assert!(ws.tree(cwd, bad).is_err(), "{bad}");
        }
        let file = ws.read_text(cwd, &[], "b.txt").unwrap();
        assert_eq!(
            (file["rel"].clone(), file["text"].clone()),
            (json!("b.txt"), json!("bee"))
        );
        assert!(
            ws.read_text(cwd, &[], "a.bin")
                .unwrap_err()
                .to_string()
                .contains("not a text file")
        );
        assert!(ws.read_text(cwd, &[], "../secret.txt").is_err());
        #[cfg(unix)]
        assert!(
            ws.read_text(cwd, &[], "link.txt").is_err(),
            "a link may not escape"
        );
        let extra = [root.to_string_lossy().into_owned()];
        assert!(
            ws.read_text(cwd, &extra, "../secret.txt").is_ok(),
            "an extra root is allowed"
        );
        let _ = std::fs::remove_dir_all(root);
    }
    #[test]
    fn listings_and_files_are_bounded_and_links_cannot_escape() {
        let root = std::env::temp_dir().join(format!("antiquota-ws-caps-{}", std::process::id()));
        let project = root.join("project");
        let many = project.join("many");
        std::fs::create_dir_all(&many).unwrap();
        for n in 0..405 {
            std::fs::write(many.join(format!("f{n:03}")), "").unwrap();
        }
        std::fs::write(project.join("big.txt"), vec![b'x'; 2_000_001]).unwrap();
        std::fs::write(project.join("latin.txt"), [b'a', 0xff, b'b']).unwrap();
        std::fs::create_dir_all(root.join("outside")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(root.join("outside"), project.join("escape")).unwrap();
        let ws = FileWorkspace;
        let cwd = project.to_str().unwrap();
        assert_eq!(
            ws.tree(cwd, "many").unwrap()["entries"]
                .as_array()
                .unwrap()
                .len(),
            400
        );
        assert!(
            ws.read_text(cwd, &[], "big.txt")
                .unwrap_err()
                .to_string()
                .contains("over 2 MB")
        );
        assert_eq!(
            ws.read_text(cwd, &[], "latin.txt").unwrap()["text"],
            "a\u{fffd}b"
        );
        #[cfg(unix)]
        assert!(
            ws.tree(cwd, "escape").is_err(),
            "a linked folder may not escape"
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
