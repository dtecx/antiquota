//! Proves where state lives and the one-time move from ~/.agyloop, through the real CLI.
use crate::support::binary;
use std::{fs, path::PathBuf, process::Command};

#[test]
fn the_cli_finds_its_folder_and_moves_the_old_one_once() {
    let owner = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).unwrap();
    let root = PathBuf::from(owner)
        .join("Desktop/test/antiquota-rust-contract")
        .join(format!("storage-{}", std::process::id()));
    let home = root.join("home");
    fs::create_dir_all(home.join(".agyloop/sessions/abc")).unwrap();
    fs::write(home.join(".agyloop/sessions/abc/meta.json"), "{}").unwrap();
    let run = |dir: Option<&PathBuf>| {
        let mut command = Command::new(binary("antiquota"));
        command
            .arg("accounts")
            .env("ANTIQUOTA_HOME", &home)
            // The account's own HOME must not decide where antiquota keeps its state.
            .env("HOME", root.join("slot-home"))
            .env("ANTIQUOTA_NO_USAGE_LINK", "1")
            .env_remove("ANTIQUOTA_DIR");
        if let Some(dir) = dir {
            command.env("ANTIQUOTA_DIR", dir);
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).into_owned()
    };
    run(None);
    let moved = home.join(".antiquota/sessions/abc/meta.json");
    assert!(moved.is_file(), "the old folder moved to ~/.antiquota");
    #[cfg(unix)]
    assert_eq!(
        fs::read_link(home.join(".agyloop")).unwrap(),
        home.join(".antiquota")
    );
    assert!(!root.join("slot-home/.antiquota").exists());
    run(None);
    assert!(moved.is_file(), "a second start changes nothing");
    let custom = root.join("custom");
    let token = custom.join("slots/x@example.com/.gemini/antigravity-cli/antigravity-oauth-token");
    fs::create_dir_all(token.parent().unwrap()).unwrap();
    fs::write(&token, "synthetic").unwrap();
    assert!(
        run(Some(&custom)).contains("x@example.com"),
        "ANTIQUOTA_DIR replaces ~/.antiquota"
    );
    assert!(!run(None).contains("x@example.com"));
    let _ = fs::remove_dir_all(root);
}
