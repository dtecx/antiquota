//! Proves token isolation, pause markers, and leases without real accounts.
use super::*;
#[test]
fn leases_are_exclusive_and_paused_slots_are_skipped() {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).unwrap();
    let root = PathBuf::from(home)
        .join("Desktop/test/antiquota-accounts")
        .join(format!(
            "{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    let auth = root.join("slots/one@example.com/.gemini/antigravity-cli/antigravity-oauth-token");
    fs::create_dir_all(auth.parent().unwrap()).unwrap();
    fs::write(&auth, "synthetic").unwrap();
    let accounts = FileAccounts::new(root.clone());
    let lease = accounts.acquire(None).unwrap().unwrap();
    assert_eq!(lease.slot().name, "one@example.com");
    assert!(accounts.acquire(None).unwrap().is_none());
    assert!(accounts.accounts().unwrap()[0].busy);
    drop(lease);
    fs::create_dir_all(root.join("state/paused")).unwrap();
    fs::write(root.join("state/paused/one@example.com"), "").unwrap();
    assert!(accounts.acquire(None).unwrap().is_none());
    assert_eq!(fs::read_to_string(&auth).unwrap(), "synthetic");
    fs::remove_dir_all(root).unwrap();
}
