//! Owns disposable native-agent fixtures and their emergency process cleanup.
#![allow(dead_code)]
use std::{fs, path::PathBuf, process::Command, sync::OnceLock, time::Duration};
pub struct Sandbox {
    pub root: PathBuf,
    pub executable: PathBuf,
}
impl Sandbox {
    pub fn new(label: &str) -> Self {
        let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
            .expect("test owner home");
        let root = PathBuf::from(home).join("Desktop/test").join(format!(
            "antiquota-lifecycle-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let executable = root.join(if cfg!(windows) {
            "fake-agy.exe"
        } else {
            "fake-agy"
        });
        fs::copy(binary(), &executable).unwrap();
        Self { root, executable }
    }
    pub fn token_home(&self) -> PathBuf {
        let home = self.root.join("slots/one@example.com");
        let token = home.join(".gemini/antigravity-cli/antigravity-oauth-token");
        fs::create_dir_all(token.parent().unwrap()).unwrap();
        fs::write(token, "synthetic isolated token").unwrap();
        home
    }
    pub fn wait_for(&self, name: &str) {
        let until = std::time::Instant::now() + Duration::from_secs(5);
        while !self.root.join(name).is_file() {
            assert!(std::time::Instant::now() < until, "missing fixture {name}");
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    pub fn kill_recorded(&self, name: &str) {
        if let Ok(pid) = fs::read_to_string(self.root.join(name)) {
            kill_pid(pid.trim());
        }
    }
}
fn binary() -> &'static PathBuf {
    static BINARY: OnceLock<PathBuf> = OnceLock::new();
    BINARY.get_or_init(|| {
        let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).unwrap();
        let directory = PathBuf::from(home).join("Desktop/test").join(format!(
            "antiquota-lifecycle-binary-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&directory).unwrap();
        let source = directory.join("fixture.rs");
        let binary = directory.join(if cfg!(windows) {
            "fake-agy.exe"
        } else {
            "fake-agy"
        });
        fs::write(source.clone(), include_str!("descendant.rs")).unwrap();
        let output = Command::new("rustc")
            .args(["--edition=2024", "-o"])
            .arg(&binary)
            .arg(source)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        binary
    })
}
fn kill_pid(pid: &str) {
    if pid.parse::<u32>().is_err() {
        return;
    }
    #[cfg(unix)]
    let _ = Command::new("kill").args(["-KILL", pid]).output();
    #[cfg(windows)]
    let _ = Command::new("taskkill")
        .args(["/PID", pid, "/T", "/F"])
        .output();
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        self.kill_recorded("runner-pid");
        self.kill_recorded("descendant-ready");
        let _ = fs::remove_dir_all(&self.root);
    }
}
