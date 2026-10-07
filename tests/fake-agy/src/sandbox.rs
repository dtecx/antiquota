//! Guard fixture writes and record only synthetic invocation metadata.
use fake_agy::FakeError;
use serde_json::json;
use std::{
    env,
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
};

pub fn isolated(path: &Path) -> Result<(), FakeError> {
    let lexical = |path: &Path| {
        let parts: Vec<_> = path.components().collect();
        path.is_absolute()
            && parts.windows(2).any(|p| {
                p[0] == Component::Normal("Desktop".as_ref())
                    && p[1] == Component::Normal("test".as_ref())
            })
            && !parts.contains(&Component::ParentDir)
    };
    if !lexical(path) {
        return Err(FakeError::Isolation(path.display().to_string()));
    }
    let mut ancestor = path;
    while !ancestor.exists() {
        ancestor = ancestor
            .parent()
            .ok_or_else(|| FakeError::Isolation(path.display().to_string()))?;
    }
    if !lexical(&ancestor.canonicalize()?) {
        return Err(FakeError::Isolation("symlink escapes Desktop/test".into()));
    }
    Ok(())
}

pub fn check() -> Result<(), FakeError> {
    for key in ["ANTIQUOTA_HOME", "ANTIQUOTA_DIR", "HOME"] {
        let path =
            env::var_os(key).ok_or_else(|| FakeError::Isolation(format!("missing {key}")))?;
        isolated(Path::new(&path))?;
    }
    if env::var("ANTIQUOTA_NO_USAGE_LINK").as_deref() != Ok("1") {
        return Err(FakeError::Isolation(
            "ANTIQUOTA_NO_USAGE_LINK must be 1".into(),
        ));
    }
    Ok(())
}

pub fn record(args: &[String]) -> Result<(), FakeError> {
    let Some(path) = env::var_os("FAKE_AGY_RECORD").map(PathBuf::from) else {
        return Ok(());
    };
    isolated(&path)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
        isolated(&parent.canonicalize()?)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    let row = json!({"argv":args,"cwd":env::current_dir()?.to_string_lossy(),
        "home":env::var("HOME").unwrap_or_default(),"slot":env::var("ANTIQUOTA_SLOT").unwrap_or_default(),
        "session":env::var("ANTIQUOTA_SESSION").unwrap_or_default(),"ssh_connection_present":env::var_os("SSH_CONNECTION").is_some()});
    writeln!(file, "{row}")?;
    Ok(())
}

pub fn next_turn() -> Result<u64, FakeError> {
    let dir = env::current_dir()?;
    isolated(&dir.canonicalize()?)?;
    let id = env::var("ANTIQUOTA_SESSION").unwrap_or_else(|_| "standalone".into());
    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(FakeError::Isolation("invalid synthetic session id".into()));
    }
    let path = dir.join(format!(".fake-agy-{id}.turns"));
    let turn = fs::read_to_string(&path)
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0)
        + 1;
    fs::write(path, turn.to_string())?;
    Ok(turn)
}
