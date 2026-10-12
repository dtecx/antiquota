//! Owns explicitly staged terminal attachments and private, non-overwriting session exports.
use crate::{
    engine::{Engine, component},
    terminal::Terminal,
};
use antiquota_core::{Attachment, Session};
use anyhow::Context;
use reqwest::Method;
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

fn path(cwd: &str, arg: &str) -> anyhow::Result<PathBuf> {
    anyhow::ensure!(
        !arg.is_empty(),
        "provide a file path relative to the session folder or an absolute path"
    );
    let text = arg
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| arg.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
        .unwrap_or(arg);
    anyhow::ensure!(!text.is_empty(), "provide a file path");
    Ok(Path::new(cwd).join(text))
}

fn read_file(path: &Path) -> anyhow::Result<Vec<u8>> {
    let metadata = path.metadata().context("the attachment is unavailable")?;
    anyhow::ensure!(metadata.is_file(), "attachments must be regular files");
    let limit = antiquota_app::RECORDING_LIMIT;
    anyhow::ensure!(
        metadata.len() <= limit as u64,
        "attachments are limited to 25 MB"
    );
    let mut bytes = vec![];
    File::open(path)?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)?;
    anyhow::ensure!(bytes.len() <= limit, "attachments are limited to 25 MB");
    Ok(bytes)
}

pub async fn run(
    engine: &Engine,
    meta: &Session,
    terminal: &Terminal,
    name: &str,
    arg: &str,
    pending: &mut Vec<Attachment>,
) -> anyhow::Result<()> {
    match name {
        "/attach" => {
            let path = path(&meta.cwd, arg)?;
            let filename = path
                .file_name()
                .and_then(|s| s.to_str())
                .context("the filename is not valid Unicode")?;
            let bytes = read_file(&path)?;
            let attachment: Attachment = serde_json::from_value(
                Engine::execute(
                    engine
                        .request(Method::POST, &format!("/api/sessions/{}/attach", meta.id))
                        .header("X-Filename", component(filename))
                        .body(bytes),
                )
                .await?,
            )?;
            terminal.line(&format!(
                "Staged {} ({} bytes) for the next message.",
                attachment.name,
                attachment.size.unwrap_or(0)
            ))?;
            pending.push(attachment);
        }
        "/attachments" => {
            anyhow::ensure!(arg.is_empty(), "/attachments takes no arguments");
            if pending.is_empty() {
                terminal.line("No files staged for the next message.")?;
            }
            for (n, file) in pending.iter().enumerate() {
                terminal.line(&format!(
                    "{}. {} ({} bytes)",
                    n + 1,
                    file.name,
                    file.size.unwrap_or(0)
                ))?;
            }
        }
        "/detach" => {
            if arg == "all" {
                pending.clear();
            } else {
                let n = arg
                    .parse::<usize>()
                    .ok()
                    .filter(|n| *n > 0 && *n <= pending.len())
                    .context("use a number from /attachments or /detach all")?;
                pending.remove(n - 1);
            }
            terminal
                .line("Removed from the next message; uploaded copies remain with the session.")?;
        }
        "/export" => {
            let path = path(&meta.cwd, arg)?;
            let saved = engine.saved(&meta.id).await?;
            let mut bytes = serde_json::to_vec_pretty(&saved)?;
            bytes.push(b'\n');
            export(&path, &bytes)?;
            terminal.line(&format!("Exported full session JSON to {}", path.display()))?;
        }
        _ => unreachable!("only context commands are dispatched here"),
    }
    Ok(())
}

fn export(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .context("cannot create the export; existing files are never overwritten")?;
    if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
        drop(file);
        let _ = std::fs::remove_file(path);
        return Err(error).context("the export did not finish");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths_and_http_components_preserve_spaces_unicode_and_literal_percent() {
        assert_eq!(
            path("project", "'a b.txt'").unwrap(),
            Path::new("project/a b.txt")
        );
        assert_eq!(
            path("project", "\"λ.txt\"").unwrap(),
            Path::new("project/λ.txt")
        );
        assert!(path("project", "").is_err() && path("project", "\"\"").is_err());
        assert_eq!(component("λ %2F#?.txt"), "%CE%BB%20%252F%23%3F.txt");
    }
}
