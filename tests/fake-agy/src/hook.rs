//! Invoke only a supplied fixture or antiquota hook; never execute tool commands.
use fake_agy::{FakeError, Script};
use serde_json::Value;
use std::{
    io::{Read, Write},
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

pub fn invoke(script: &Script) -> Result<Option<Value>, FakeError> {
    let Some(program) = script.hook_command.first() else {
        return Ok(None);
    };
    let name = Path::new(program)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    if !matches!(
        name,
        "antiquota" | "antiquota.exe" | "fake-hook" | "fake-hook.exe"
    ) {
        return Err(FakeError::Scenario(
            "hook command must name antiquota or fake-hook".into(),
        ));
    }
    let mut child = Command::new(program)
        .args(&script.hook_command[1..])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let payload = serde_json::to_string(&script.hook_payload)?;
    // A hook may answer without reading its input and close the pipe before the write ends.
    match child.stdin.take().unwrap().write_all(payload.as_bytes()) {
        Err(e) if e.kind() != std::io::ErrorKind::BrokenPipe => return Err(e.into()),
        _ => {}
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if child.try_wait()?.is_some() {
            break;
        }
        if Instant::now() >= deadline {
            child.kill()?;
            child.wait()?;
            return Err(FakeError::Scenario("fixture hook timed out".into()));
        }
        thread::sleep(Duration::from_millis(10));
    }
    let mut output = String::new();
    child
        .stdout
        .take()
        .unwrap()
        .take(65_536)
        .read_to_string(&mut output)?;
    Ok(Some(serde_json::from_str(output.trim())?))
}
