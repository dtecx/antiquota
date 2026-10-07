//! Owns synthetic metadata descendants for inherited-pipe and cancellation proof.
use fake_agy::{FakeError, metadata::Metadata};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::Duration,
};

pub fn handle(args: &[String]) -> Result<bool, FakeError> {
    if args.first().map(String::as_str) != Some("--fake-inherited-pipes") {
        return Ok(false);
    }
    let delay = args
        .get(1)
        .ok_or_else(|| FakeError::Scenario("missing descendant delay".into()))?
        .parse::<u64>()
        .map_err(|_| FakeError::Scenario("invalid descendant delay".into()))?;
    let marker = args.get(2).map(PathBuf::from);
    if let Some(marker) = &marker {
        super::sandbox::isolated(marker)?;
    }
    thread::sleep(Duration::from_millis(delay));
    if let Some(marker) = marker {
        fs::create_dir_all(marker.parent().unwrap())?;
        fs::write(marker, "metadata descendant survived\n")?;
    }
    Ok(true)
}
pub fn spawn(metadata: &Metadata) -> Result<(), FakeError> {
    let Some(delay) = metadata.inherited_pipe_ms else {
        return Ok(());
    };
    let mut child = Command::new(std::env::current_exe()?);
    child.args(["--fake-inherited-pipes", &delay.to_string()]);
    if let Some(marker) = &metadata.descendant_marker {
        super::sandbox::isolated(std::path::Path::new(marker))?;
        child.arg(marker);
    }
    child
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;
    Ok(())
}
