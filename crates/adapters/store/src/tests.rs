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
