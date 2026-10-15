//! Owns keeping the computer awake while agents or jobs run (the display may still sleep):
//! `caffeinate` on macOS, `systemd-inhibit` on Linux when available, and on Windows a hidden
//! PowerShell that holds `SetThreadExecutionState` until it is killed.
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
    } else if cfg!(windows) {
        let mut c = std::process::Command::new("powershell");
        c.args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Add-Type -Name P -Namespace W -MemberDefinition '[DllImport(\"kernel32.dll\")] public static extern uint SetThreadExecutionState(uint f);'; [W.P]::SetThreadExecutionState(0x80000001) | Out-Null; while ($true) { Start-Sleep -Seconds 3600 }",
        ]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: no console window flashes up
        }
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
