//! Owns session attachments on disk and temporary dictation recordings.
use crate::{FileStore, io_error};
use antiquota_core::{Attachment, PortError};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// PNG, JPEG, GIF, WebP, BMP and HEIC files are images agy can look at.
pub fn is_image(name: &str) -> bool {
    let lower = name.to_lowercase();
    [".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp", ".heic"]
        .iter()
        .any(|ext| lower.ends_with(ext))
}

/// A safe file name: other characters become `_`, outer spaces and dots go, "file" when empty.
pub fn safe_name(filename: &str) -> String {
    let base = filename.rsplit(['/', '\\']).next().unwrap_or("");
    let mut out = String::new();
    let mut replaced = false;
    for c in base.chars() {
        if c.is_alphanumeric() || "_.- ".contains(c) {
            out.push(c);
            replaced = false;
        } else if !replaced {
            out.push('_');
            replaced = true;
        }
    }
    let trimmed = out.trim_matches([' ', '.']);
    if trimmed.is_empty() {
        "file".into()
    } else {
        trimmed.into()
    }
}

fn split(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(at) if at > 0 => (&name[..at], &name[at..]),
        _ => (name, ""),
    }
}

impl FileStore {
    pub(crate) fn attachment_dir(&self, sid: &str) -> Result<PathBuf, PortError> {
        Ok(self.session_dir(sid)?.join("attachments"))
    }

    pub(crate) fn store_attachment(
        &self,
        sid: &str,
        filename: &str,
        data: &[u8],
    ) -> Result<Attachment, PortError> {
        let dir = self.attachment_dir(sid)?;
        fs::create_dir_all(&dir).map_err(io_error)?;
        let name = safe_name(filename);
        let (stem, ext) = split(&name);
        let mut path = dir.join(&name);
        let mut n = 1;
        while path.exists() {
            n += 1;
            path = dir.join(format!("{stem}-{n}{ext}"));
        }
        fs::write(&path, data).map_err(io_error)?;
        let stored = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok(Attachment {
            image: is_image(&stored),
            name: stored,
            path: path.to_string_lossy().into_owned(),
            size: Some(data.len() as u64),
        })
    }

    pub(crate) fn load_attachment(
        &self,
        sid: &str,
        name: &str,
    ) -> Result<Option<Vec<u8>>, PortError> {
        let Ok(dir) = crate::canonical(self.attachment_dir(sid)?) else {
            return Ok(None);
        };
        let Ok(path) = crate::canonical(dir.join(name)) else {
            return Ok(None);
        };
        if !path.starts_with(&dir) || path == dir || !path.is_file() {
            return Ok(None);
        }
        fs::read(path).map(Some).map_err(io_error)
    }

    pub(crate) fn existing_dirs(&self, sid: &str) -> Vec<String> {
        ["attachments", "jobs"]
            .iter()
            .filter_map(|name| self.session_dir(sid).ok().map(|d| d.join(name)))
            .filter(|d| d.is_dir())
            .map(|d| d.to_string_lossy().into_owned())
            .collect()
    }

    pub(crate) fn write_recording(
        &self,
        data: &[u8],
        extension: &str,
    ) -> Result<(String, String), PortError> {
        let folder = self.root.join("state/dictation");
        fs::create_dir_all(&folder).map_err(io_error)?;
        let ext: String = extension
            .to_lowercase()
            .chars()
            .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
            .take(5)
            .collect();
        let ext = if ext.is_empty() { "wav".into() } else { ext };
        let id = uuid_like();
        let path = folder.join(format!("{id}.{ext}"));
        fs::write(&path, data).map_err(io_error)?;
        Ok((
            folder.to_string_lossy().into_owned(),
            path.to_string_lossy().into_owned(),
        ))
    }
}

fn uuid_like() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!(
        "{:010x}",
        (nanos ^ u128::from(std::process::id())) & 0xff_ffff_ffff
    )
}

pub(crate) fn remove(path: &str) {
    let _ = fs::remove_file(Path::new(path));
}

#[cfg(test)]
mod tests {
    use super::*;
    use antiquota_core::SessionStore;
    #[test]
    fn names_are_sanitized_and_never_collide() {
        assert_eq!(safe_name("../../etc/passwd"), "passwd");
        assert_eq!(safe_name(r"C:\x\report (final).pdf"), "report _final_.pdf");
        assert_eq!(safe_name("  .hidden. "), "hidden");
        assert_eq!(safe_name("Résumé été.txt"), "Résumé été.txt");
        assert_eq!(safe_name("???"), "_");
        assert_eq!(safe_name(""), "file");
        assert!(is_image("Shot.PNG") && is_image("a.heic") && !is_image("a.txt"));
        let root = std::env::temp_dir().join(format!("antiquota-attach-{}", std::process::id()));
        let store = FileStore::open(root.clone()).unwrap();
        fs::create_dir_all(root.join("sessions/s1")).unwrap();
        let first = store.store_attachment("s1", "note.txt", b"one").unwrap();
        let second = store.store_attachment("s1", "note.txt", b"two").unwrap();
        let third = store.store_attachment("s1", "note.txt", b"three").unwrap();
        assert_eq!(
            (
                first.name.as_str(),
                second.name.as_str(),
                third.name.as_str()
            ),
            ("note.txt", "note-2.txt", "note-3.txt")
        );
        assert_eq!(first.size, Some(3));
        assert_eq!(
            store.load_attachment("s1", "note-2.txt").unwrap(),
            Some(b"two".to_vec())
        );
        assert_eq!(store.load_attachment("s1", "../meta.json").unwrap(), None);
        assert_eq!(store.load_attachment("s1", "missing").unwrap(), None);
        let dirs: Vec<PathBuf> = store
            .existing_dirs("s1")
            .iter()
            .map(PathBuf::from)
            .collect();
        assert_eq!(dirs, [root.join("sessions").join("s1").join("attachments")]);
        let inside = root.join("sessions/s1/attachments/note.txt");
        let outside = root.join("outside.txt");
        fs::write(&outside, "secret").unwrap();
        // Only Unix adds an escaping link below.
        #[cfg_attr(not(unix), allow(unused_mut))]
        let mut offered = vec![
            inside.to_string_lossy().into_owned(),
            outside.to_string_lossy().into_owned(),
            root.join("sessions/s1/attachments")
                .to_string_lossy()
                .into_owned(),
        ];
        #[cfg(unix)]
        {
            let link = root.join("sessions/s1/attachments/link.txt");
            std::os::unix::fs::symlink(&outside, &link).unwrap();
            offered.push(link.to_string_lossy().into_owned());
            assert_eq!(store.load_attachment("s1", "link.txt").unwrap(), None);
        }
        let kept = store.attachments("s1", &offered).unwrap();
        assert_eq!(
            kept.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(),
            ["note.txt"],
            "outside files, folders and escaping links are dropped"
        );
        let (folder, path) = store.write_recording(b"RIFF", "Web M!longer").unwrap();
        assert!(path.starts_with(&folder) && path.ends_with(".webml"));
        remove(&path);
        assert!(!Path::new(&path).exists());
        let _ = fs::remove_dir_all(root);
    }
}
