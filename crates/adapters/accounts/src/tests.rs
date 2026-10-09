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
fn concurrent_permission_grants_keep_every_rule_and_opaque_setting() {
    let sandbox = Sandbox::new();
    let source = r#"{"permissions":{"allow":["command(private)"],"deny":["command(rm)"]}}"#;
    sandbox.write("home/.gemini/antigravity-cli/settings.json", source);
    sandbox.write(
        "data/stack/.gemini/antigravity-cli/settings.json",
        r#"{"permissions":{"allow":["command(ls)"],"deny":["command(rm)"],"future":{"opaque":[false,null,123456789012345678901234567890]}},"general":{"keep":true},"extension":{"value":"unchanged"}}"#,
    );
    let accounts = sandbox.accounts();
    let path = sandbox
        .0
        .join("data/stack/.gemini/antigravity-cli/settings.json");
    let original: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let barrier = Arc::new(std::sync::Barrier::new(24));
    let mut workers = vec![];
    for index in 0..24 {
        let accounts = accounts.clone();
        let barrier = barrier.clone();
        workers.push(std::thread::spawn(move || {
            let rule = format!("command(git fixture-{index})");
            barrier.wait();
            if index % 2 == 0 {
                accounts
                    .grant_permissions(vec![rule.clone(), rule, "command(ls)".into()])
                    .unwrap();
            } else {
                accounts.add_allow_rule(&rule).unwrap();
                accounts.add_allow_rule(&rule).unwrap();
            }
        }));
    }
    for worker in workers {
        worker.join().unwrap();
    }
    let stored: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let allow = stored["permissions"]["allow"].as_array().unwrap();
    assert_eq!(allow.len(), 25, "every rule is kept exactly once");
    assert_eq!(allow[0], "command(ls)");
    for index in 0..24 {
        assert!(allow.contains(&serde_json::json!(format!("command(git fixture-{index})"))));
    }
    let mut expected = original;
    expected["permissions"]["allow"] = stored["permissions"]["allow"].clone();
    assert_eq!(
        stored, expected,
        "deny rules and all extensions are preserved"
    );
    assert_eq!(
        fs::read_to_string(sandbox.0.join("home/.gemini/antigravity-cli/settings.json")).unwrap(),
        source,
        "the source agy configuration is never changed"
    );
}

#[test]
fn permission_grants_validate_the_whole_batch_before_writing() {
    let sandbox = Sandbox::new();
    sandbox.write(
        "data/stack/.gemini/antigravity-cli/settings.json",
        r#"{"permissions":{"allow":["command(ls)"],"deny":["command(rm)"],"future":true},"general":{"keep":true}}"#,
    );
    let accounts = sandbox.accounts();
    let path = sandbox
        .0
        .join("data/stack/.gemini/antigravity-cli/settings.json");
    let original = fs::read(&path).unwrap();
    for invalid in [
        "".to_owned(),
        "x".repeat(300),
        "command(/bin/bash -c)".into(),
        "command(/bin/bas[h])".into(),
        "command(/bin/{ba,z}sh)".into(),
    ] {
        assert!(
            accounts
                .grant_permissions(vec!["command(cat)".into(), invalid.clone()])
                .is_err(),
            "invalid batch: {invalid}"
        );
        assert_eq!(
            fs::read(&path).unwrap(),
            original,
            "no partial grant: {invalid}"
        );
    }
    let granted = accounts
        .grant_permissions(vec![
            "command(ls)".into(),
            "command(cat)".into(),
            "command(cat)".into(),
        ])
        .unwrap();
    assert_eq!(
        granted["allow"],
        serde_json::json!(["command(ls)", "command(cat)"])
    );
    assert_eq!(granted["deny"], serde_json::json!(["command(rm)"]));
    assert_eq!(granted["future"], true);
    for malformed in [
        "{invalid",
        "null",
        "false",
        "[]",
        r#"{"permissions":false}"#,
        r#"{"permissions":[]}"#,
        r#"{"permissions":{"allow":false,"deny":["command(rm)"]}}"#,
        r#"{"permissions":{"allow":["command(ls)",false],"deny":["command(rm)"]}}"#,
    ] {
        sandbox.write(
            "data/stack/.gemini/antigravity-cli/settings.json",
            malformed,
        );
        assert!(
            accounts
                .grant_permissions(vec!["command(cat)".into()])
                .is_err(),
            "{malformed}"
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), malformed);
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
#[test]
fn routing_history_trims_above_two_megabytes_to_the_newest_five_thousand_lines() {
    let s = Sandbox::new();
    let accounts = s.accounts();
    let path = s.0.join("data/state/routing.jsonl");
    let entry = RoutingEntry {
        ts: 7.0,
        event: "resumed".into(),
        slot: Some("one@example.com".into()),
        ..Default::default()
    };
    s.write("data/state/routing.jsonl", "");
    accounts.record_route(&entry).unwrap();
    let appended = fs::metadata(&path).unwrap().len() as usize;
    let line = |i: usize, pad: usize| {
        format!(
            "{{\"ts\":{i},\"event\":\"pick\",\"why\":\"{}\"}}\n",
            "x".repeat(pad)
        )
    };
    // The file ends exactly at 2 MB after the append, then one byte above it.
    for (size, trimmed) in [(2_000_000, false), (2_000_001, true)] {
        let mut lines: Vec<String> = (0..6_000).map(|i| line(i, 0)).collect();
        let short: usize = lines.iter().map(String::len).sum();
        lines[0] = line(0, size - appended - short);
        s.write("data/state/routing.jsonl", &lines.concat());
        accounts.record_route(&entry).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        let kept: Vec<_> = text.lines().collect();
        if trimmed {
            assert_eq!(kept.len(), 5_000);
            assert_eq!(kept[0], lines[1_001].trim_end());
        } else {
            assert_eq!(text.len(), 2_000_000);
            assert_eq!(kept.len(), 6_001);
        }
        let last: RoutingEntry = serde_json::from_str(kept.last().unwrap()).unwrap();
        assert_eq!(last, entry);
    }
    // A read without a count returns the newest 200 entries in stored order.
    let rows: String = (0..300)
        .map(|i| format!("{{\"ts\":{i},\"event\":\"pick\"}}\n"))
        .collect();
    s.write("data/state/routing.jsonl", &rows);
    let tail = accounts.recent_routes().unwrap();
    assert_eq!(tail.len(), 200);
    assert_eq!((tail[0].ts, tail[199].ts), (100.0, 299.0));
    assert_eq!(accounts.routes(5).unwrap()[0].ts, 295.0);
}
#[test]
fn today_starts_at_local_midnight() {
    use chrono::{Local, TimeZone};
    let s = Sandbox::new();
    // Noon keeps the day well inside any daylight-saving change.
    let noon = Local
        .with_ymd_and_hms(2026, 3, 18, 12, 0, 0)
        .single()
        .unwrap()
        .timestamp() as f64;
    let midnight = Local
        .with_ymd_and_hms(2026, 3, 18, 0, 0, 0)
        .single()
        .unwrap()
        .timestamp() as f64;
    let row = |t: f64, input: u64| format!("{{\"t\":{t},\"slot\":\"one\",\"input\":{input}}}\n");
    s.write(
        "data/state/usage.jsonl",
        &[row(midnight - 1.0, 1), row(midnight, 10), row(noon, 100)].concat(),
    );
    let totals = overview::usage_today(&s.accounts(), noon).unwrap();
    let today = totals.get("one").unwrap();
    assert_eq!((today.input, today.turns), (Some(110), Some(2)));
}
