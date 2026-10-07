//! Owns keeping the computer awake while agents or jobs run (the display may still sleep):
//! `caffeinate` on macOS and `systemd-inhibit` on Linux when available.
use antiquota_core::Awake;
use std::{process::Child, sync::Mutex};

#[derive(Default)]
pub struct KeepAwake {
    held: Mutex<Option<Option<Child>>>,
}

fn inhibitor() -> Option<Child> {
    let mut command = if cfg!(target_os = "macos") {
        let mut c = std::process::Command::new("caffeinate");
        c.args(["-i", "-w", &std::process::id().to_string()]);
        c
    } else if cfg!(target_os = "linux") {
        let mut c = std::process::Command::new("systemd-inhibit");
        c.args([
            "--what=idle:sleep",
            "--who=antiquota",
            "--why=agents are running",
            "sleep",
            "infinity",
        ]);
        c
    } else {
        return None;
    };
    command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()
}

impl Awake for KeepAwake {
    fn hold(&self) {
        if let Ok(mut held) = self.held.lock()
            && held.is_none()
        {
            *held = Some(inhibitor());
        }
    }
    fn release(&self) {
        if let Ok(mut held) = self.held.lock()
            && let Some(Some(mut child)) = held.take()
        {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
impl Drop for KeepAwake {
    fn drop(&mut self) {
        self.release();
    }
}
