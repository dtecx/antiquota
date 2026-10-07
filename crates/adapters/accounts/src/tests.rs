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

struct Sandbox(PathBuf);
impl Sandbox {
    fn new() -> Self {
        let owner = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).unwrap();
        let root = PathBuf::from(owner)
            .join("Desktop/test/antiquota-account-units")
            .join(uuid::Uuid::new_v4().to_string());
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }
    fn write(&self, path: &str, text: &str) {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fn accounts(&self) -> FileAccounts {
        FileAccounts::with_runtime(
            self.0.join("data"),
            self.0.join("home"),
            "never-executed".into(),
        )
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn clone_and_sync_preserve_private_identity_history_and_unknown_config() {
    let s = Sandbox::new();
    s.write(
        "home/.gemini/config/config.json",
        r#"{"future":{"huge":123456789012345678901234567890}}"#,
    );
    s.write(
        "home/.gemini/antigravity-cli/antigravity-oauth-token",
        "source-private",
    );
    s.write(
        "home/.gemini/antigravity-cli/jetski_state.pbtxt",
        "source-private-state",
    );
    s.write(
        "home/.gemini/antigravity-cli/conversations/source",
        "source-history",
    );
    s.write("home/.gemini/GEMINI.md", "source rules");
    s.write(
        "data/slots/one@example.com/.gemini/antigravity-cli/antigravity-oauth-token",
        "slot-private",
    );
    s.write(
        "data/stack/.gemini/antigravity-cli/conversations/owned",
        "owned-history",
    );
    let accounts = s.accounts();
    accounts.seed_stack(true).unwrap();
    let slot = accounts.slots().unwrap().remove(0);
    accounts.sync(&slot).unwrap();
    assert_eq!(
        fs::read_to_string(PathBuf::from(slot.home).join(AUTH)).unwrap(),
        "slot-private"
    );
    assert_eq!(
        fs::read_to_string(s.0.join("home/.gemini/antigravity-cli/antigravity-oauth-token"))
            .unwrap(),
        "source-private"
    );
    assert!(
        !s.0.join("data/stack/.gemini/antigravity-cli/antigravity-oauth-token")
            .exists()
    );
    assert!(
        !s.0.join("data/stack/.gemini/antigravity-cli/jetski_state.pbtxt")
            .exists()
    );
    assert!(
        s.0.join("data/stack/.gemini/antigravity-cli/conversations/owned")
            .is_file()
    );
    assert!(
        !s.0.join("data/stack/.gemini/antigravity-cli/conversations/source")
            .exists()
    );
    let config: serde_json::Value = serde_json::from_slice(
        &fs::read(s.0.join("data/stack/.gemini/config/config.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        config["future"]["huge"].to_string(),
        "123456789012345678901234567890"
    );
}
#[test]
fn reseed_protects_the_owned_hook_and_quota_updates_preserve_opaque_extensions() {
    let s = Sandbox::new();
    s.write(
        "home/.gemini/config/hooks.json",
        r#"{"other":{"enabled":true,"future":1}}"#,
    );
    s.write(
        "data/stack/.gemini/config/hooks.json",
        r#"{"antiquota":{"enabled":true,"PreToolUse":["owned"]}}"#,
    );
    s.write("data/state/quota/one@example.com.json",r#"{"at":1,"future":123456789012345678901234567890,"quota":{"gemini":{"future-window":{"opaque":true},"weekly":[0.5,300]}}}"#);
    s.write(
        "data/slots/one@example.com/.gemini/antigravity-cli/antigravity-oauth-token",
        "slot-private",
    );
    let accounts = s.accounts();
    accounts.seed_stack(true).unwrap();
    let hooks: serde_json::Value = serde_json::from_slice(
        &fs::read(s.0.join("data/stack/.gemini/config/hooks.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        hooks["antiquota"]["PreToolUse"],
        serde_json::json!(["owned"])
    );
    assert_eq!(hooks["other"]["future"], 1);
    let quota = Quota(
        [(
            "gemini".into(),
            [("weekly".into(), (0.7, Some(500.0)))].into(),
        )]
        .into(),
    );
    accounts
        .save_quota("one@example.com", &quota, 20.0)
        .unwrap();
    let value = files::read_json(&s.0.join("data/state/quota/one@example.com.json")).unwrap();
    assert_eq!(
        value["future"].to_string(),
        "123456789012345678901234567890"
    );
    assert_eq!(
        value["quota"]["gemini"]["future-window"],
        serde_json::json!({"opaque":true})
    );
    assert_eq!(
        accounts.accounts_at(20.0).unwrap()[0].quota.0["gemini"]["weekly"],
        (0.7, Some(500.0))
    );
}
#[test]
fn disabled_quota_windows_and_exact_urls_are_parsed_without_json_suffixes() {
    let q=parse_quota(&serde_json::json!({"command":{"data":{"groups":[{"name":"Gemini","buckets":[{"window":"weekly","remaining_fraction":0.0,"reset_time":"2099-01-07T00:00:00Z"},{"window":"5h","remaining_fraction":null,"reset_time":null}]}]}}})).unwrap();
    assert_eq!(q.0["gemini"]["5h"], (0.0, None));
    assert!(q.0["gemini"]["weekly"].1.unwrap() > 0.0);
    assert_eq!(metadata::google_url(r#"{"error":"eligibility https://accounts.google.com/signin/continue?fixture=one","status":"ERROR"}"#,true).as_deref(),Some("https://accounts.google.com/signin/continue?fixture=one"));
    assert!(parse_quota(&serde_json::json!({"command":{"data":{"groups":[{"name":"Gemini","buckets":[{"window":"weekly","remaining_fraction":"NaN"}]}]}}})).is_err());
}
#[test]
fn local_day_usage_skips_malformed_rows_and_keeps_all_four_fields() {
    let s = Sandbox::new();
    s.write("data/state/usage.jsonl","{\"slot\":\"one\"}\n{\"t\":200,\"slot\":\"one\",\"input\":\"bad\"}\n{\"t\":200,\"slot\":\"one\",\"input\":3,\"output\":2,\"thinking\":1}\n");
    let accounts = s.accounts();
    let totals = overview::usage_today(&accounts, 200.0).unwrap();
    let today = totals.get("one").unwrap();
    assert_eq!(
        serde_json::to_value(today).unwrap(),
        serde_json::json!({"input":3,"output":2,"thinking":1,"turns":1})
    );
}

#[test]
fn executable_discovery_prefers_expanded_home_over_bare_path() {
    let s = Sandbox::new();
    let file = if cfg!(windows) { "agy.exe" } else { "agy" };
    s.write(&format!("home/.local/bin/{file}"), "synthetic executable");
    s.write(&format!("path/{file}"), "second executable");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for path in [
            s.0.join("home/.local/bin").join(file),
            s.0.join("path").join(file),
        ] {
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        }
    }
    let first = s.0.join("home/.local/bin");
    let second = s.0.join("path");
    assert_eq!(
        metadata::find_in_dirs(&[first.clone(), second.clone()]),
        Some(first.join(file).canonicalize().unwrap())
    );
    assert_eq!(
        metadata::find_in_dirs(std::slice::from_ref(&second)),
        Some(second.join(file).canonicalize().unwrap())
    );
    assert_eq!(metadata::find_in_dirs(&[s.0.join("missing")]), None);
}
