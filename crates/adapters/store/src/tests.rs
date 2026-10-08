//! Proves rollback-compatible recovery and usage projection at crash boundaries.
use super::*;
use antiquota_core::{Event, StepUsage, replay};
use serde_json::json;
struct Sandbox(PathBuf);
impl Sandbox {
    fn new() -> Self {
        let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).unwrap();
        let root = PathBuf::from(home)
            .join("Desktop/test/antiquota-store")
            .join(uuid::Uuid::new_v4().to_string());
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn first_meta_survives_missing_snapshot() {
    let sandbox = Sandbox::new();
    let store = FileStore::open(sandbox.0.clone()).unwrap();
    let session = Session {
        id: "abcdef123456".into(),
        title: "Recovered".into(),
        ..Default::default()
    };
    store
        .append(
            &session.id,
            &EventRecord::new(
                1.0,
                Event::Meta {
                    meta: Box::new(session.clone()),
                },
            ),
        )
        .unwrap();
    assert_eq!(store.load().unwrap()[0].0, session);
}
#[test]
fn opaque_numbers_survive_snapshot_roundtrip() {
    let sandbox = Sandbox::new();
    let store = FileStore::open(sandbox.0.clone()).unwrap();
    let session:Session=serde_json::from_str(r#"{"id":"abcdef123456","future":123456789012345678901234567890,"decimal":1.2345678901234567890123456789,"usage":{"future":123456789012345678901234567890}}"#).unwrap();
    store.save(&session).unwrap();
    let loaded = store.load().unwrap().remove(0).0;
    assert_eq!(
        serde_json::to_value(session).unwrap(),
        serde_json::to_value(loaded).unwrap()
    );
}
#[test]
fn short_token_regenerates_then_reuses() {
    let sandbox = Sandbox::new();
    let root = &sandbox.0;
    fs::create_dir_all(root.join("state")).unwrap();
    fs::write(root.join("state/gui-token"), "short").unwrap();
    let first = token(root).unwrap();
    assert!(first.len() >= 32);
    assert_eq!(first, token(root).unwrap());
}
#[test]
fn unterminated_line_is_retained_without_corrupting_next_append() {
    let sandbox = Sandbox::new();
    let path = sandbox.0.join("events.jsonl");
    fs::write(&path, b"{broken").unwrap();
    append_json(&path, &json!({"okay":true})).unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"{broken\n{\"okay\":true}\n");
    assert_eq!(read_values(&path).unwrap(), vec![json!({"okay":true})]);
}
#[test]
fn usage_recovery_is_idempotent_preserves_legacy_duplicates_and_deleted_history() {
    let sandbox = Sandbox::new();
    let store = FileStore::open(sandbox.0.clone()).unwrap();
    let id = "abcdef123456";
    let ledger = store.root.join("state/usage.jsonl");
    let legacy =
        json!({"session":id,"t":1.0,"slot":"a","model":"m","input":5,"output":1,"thinking":0});
    append_json(&ledger, &legacy).unwrap();
    append_json(&ledger, &legacy).unwrap();
    let session = Session {
        id: id.into(),
        model: "m".into(),
        ..Default::default()
    };
    let mut records = vec![EventRecord::new(
        2.0,
        Event::Meta {
            meta: Box::new(session.clone()),
        },
    )];
    store.append(id, &records[0]).unwrap();
    for _ in 0..2 {
        let timestamp = store.timestamp(id, 3.0, &records).unwrap();
        let record = EventRecord::new(
            timestamp,
            Event::TurnEnd {
                turn: records.len() as u64,
                slot: "a".into(),
                status: "SUCCESS".into(),
                usage: StepUsage {
                    input_tokens: 10,
                    total_tokens: 10,
                    ..Default::default()
                },
                secs: 0.0,
            },
        );
        store.append(id, &record).unwrap();
        records.push(record);
        store.reconcile_usage(id, &records).unwrap();
        store.reconcile_usage(id, &records).unwrap();
    }
    assert_eq!(read_values(&ledger).unwrap().len(), 4);
    assert_ne!(records[1].ts, records[2].ts);
    assert_eq!(replay(session, &records).unwrap().usage.input, 20);
    store.delete(id).unwrap();
    assert_eq!(read_values(&ledger).unwrap().len(), 4);
}
#[test]
fn timestamp_collision_never_advances_to_infinity() {
    let sandbox = Sandbox::new();
    let store = FileStore::open(sandbox.0.clone()).unwrap();
    let record = EventRecord::new(f64::MAX, Event::Deleted {});
    assert!(
        store
            .timestamp("abcdef123456", f64::MAX, &[record])
            .is_err()
    );
}
#[test]
fn settings_keep_each_readable_value_and_fall_back_per_key() {
    let sandbox = Sandbox::new();
    let read = |text: &str| {
        fs::write(sandbox.0.join("settings.json"), text).unwrap();
        read_settings(&sandbox.0).unwrap()
    };
    assert_eq!(read("not json"), antiquota_core::Settings::default());
    assert_eq!(read("[1]"), antiquota_core::Settings::default());
    let mixed = read(
        r#"{"model":"gemini-pro-high","reserve":5,"max_rounds":7.5,"keep_awake":false,
            "quota_poll":null,"auto_continue":[1],"notify":{"done":true},
            "future":{"opaque":123456789012345678901234567890}}"#,
    );
    assert_eq!(mixed.model, "gemini-pro-high");
    assert_eq!(
        (
            mixed.reserve.as_str(),
            mixed.max_rounds.as_str(),
            mixed.keep_awake.as_str()
        ),
        ("5", "7.5", "False")
    );
    assert_eq!(
        (mixed.quota_poll.as_str(), mixed.auto_continue.as_str()),
        ("10", "2")
    );
    assert_eq!(
        mixed.extra["notify"],
        json!({"done": true}),
        "unknown keys are kept as they are"
    );
    assert_eq!(
        mixed.extra["future"]["opaque"].to_string(),
        "123456789012345678901234567890"
    );
    fs::remove_file(sandbox.0.join("settings.json")).unwrap();
    assert_eq!(
        read_settings(&sandbox.0).unwrap(),
        antiquota_core::Settings::default()
    );
}
#[test]
#[cfg(unix)]
fn the_old_agyloop_folder_moves_once_with_its_links_and_paths() {
    use std::os::unix::fs::symlink;
    let sandbox = Sandbox::new();
    let home = sandbox.0.join("home");
    let (old, new) = (home.join(".agyloop"), home.join(".antiquota"));
    fs::create_dir_all(old.join("stack/.gemini/config")).unwrap();
    fs::create_dir_all(old.join("slots/one/.gemini")).unwrap();
    fs::create_dir_all(home.join(".config")).unwrap();
    symlink(
        old.join("stack/.gemini/config"),
        old.join("slots/one/.gemini/config"),
    )
    .unwrap();
    symlink("/elsewhere/kept", old.join("slots/one/foreign")).unwrap();
    let hook = format!("{{\"command\":\"{}/stack/hook.cjs\"}}", old.display());
    fs::write(old.join("stack/.gemini/config/hooks.json"), &hook).unwrap();
    symlink(
        old.join("stack/.gemini/antigravity-cli"),
        home.join(".config/antigravity"),
    )
    .unwrap();
    // An existing destination or a linked source means there is nothing to move.
    fs::create_dir_all(&new).unwrap();
    migrate_from_agyloop(&old, &new, &home).unwrap();
    assert!(old.is_dir() && !old.is_symlink());
    fs::remove_dir(&new).unwrap();

    migrate_from_agyloop(&old, &new, &home).unwrap();
    assert_eq!(
        fs::read_link(&old).unwrap(),
        new,
        "a link keeps old recorded paths working"
    );
    assert_eq!(
        fs::read_link(new.join("slots/one/.gemini/config")).unwrap(),
        new.join("stack/.gemini/config")
    );
    assert_eq!(
        fs::read_link(new.join("slots/one/foreign")).unwrap(),
        PathBuf::from("/elsewhere/kept")
    );
    assert_eq!(
        fs::read_to_string(new.join("stack/.gemini/config/hooks.json")).unwrap(),
        format!("{{\"command\":\"{}/stack/hook.cjs\"}}", new.display())
    );
    assert_eq!(
        fs::read_link(home.join(".config/antigravity")).unwrap(),
        new.join("stack/.gemini/antigravity-cli")
    );
    // A retry changes nothing.
    migrate_from_agyloop(&old, &new, &home).unwrap();
    assert_eq!(fs::read_link(&old).unwrap(), new);
}
#[test]
fn the_home_is_the_account_home_not_a_slot_home() {
    // Tests never set ANTIQUOTA_HOME for this process, so the account database decides.
    if std::env::var_os("ANTIQUOTA_HOME").is_some() {
        return;
    }
    #[cfg(unix)]
    let account = nix::unistd::User::from_uid(nix::unistd::getuid())
        .unwrap()
        .unwrap()
        .dir;
    #[cfg(windows)]
    let account = PathBuf::from(std::env::var_os("USERPROFILE").unwrap());
    assert_eq!(user_home().unwrap(), account);
}
#[test]
fn concurrent_atomic_writes_leave_one_whole_file_and_no_temporaries() {
    let sandbox = Sandbox::new();
    let path = sandbox.0.join("state/quota.json");
    std::thread::scope(|scope| {
        for writer in 0..8 {
            let path = &path;
            scope.spawn(move || {
                for round in 0..20 {
                    write_json_atomic(path, &json!({"writer": writer, "round": round})).unwrap();
                }
            });
        }
    });
    let value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(value["round"], 19);
    assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        drop(lock_file(&sandbox.0.join("state/locks/one.lock")).unwrap());
        let mode = fs::metadata(sandbox.0.join("state/locks/one.lock"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
#[test]
fn only_turns_with_tokens_reach_the_ledger_with_their_account_model_and_session() {
    let sandbox = Sandbox::new();
    let store = FileStore::open(sandbox.0.clone()).unwrap();
    let session = Session {
        id: "s".into(),
        model: "gemini-pro-high".into(),
        ..Default::default()
    };
    let end = |turn, input, total, thinking| Event::TurnEnd {
        turn,
        slot: "two@example.com".into(),
        status: "SUCCESS".into(),
        usage: StepUsage {
            input_tokens: input,
            output_tokens: 2,
            total_tokens: total,
            thinking_tokens: thinking,
            ..Default::default()
        },
        secs: 1.0,
    };
    let records = vec![
        EventRecord::new(
            1.0,
            Event::Meta {
                meta: Box::new(session.clone()),
            },
        ),
        EventRecord::new(
            2.0,
            Event::TurnStart {
                turn: 1,
                slot: "two@example.com".into(),
                model: "gemini-3.8-flash-high".into(),
            },
        ),
        EventRecord::new(3.0, end(1, 0, 0, 0)),
        EventRecord::new(
            4.0,
            Event::TurnStart {
                turn: 2,
                slot: "two@example.com".into(),
                model: "gemini-3.8-flash-high".into(),
            },
        ),
        EventRecord::new(5.0, end(2, 3, 9, 4)),
    ];
    store.reconcile_usage("s", &records).unwrap();
    let rows = read_values(&sandbox.0.join("state/usage.jsonl")).unwrap();
    assert_eq!(
        rows,
        [
            json!({"t":5.0,"slot":"two@example.com","model":"gemini-3.8-flash-high","session":"s","input":3,"output":2,"thinking":4})
        ],
        "a turn without tokens writes no row"
    );
}
