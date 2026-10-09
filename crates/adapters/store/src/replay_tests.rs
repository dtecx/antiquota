//! Properties of the event log: what is appended replays the same after a restart, and a write
//! cut off part way loses only the unfinished record.
use super::*;
use antiquota_core::{Event, StepUsage, replay};
use proptest::prelude::*;
use serde_json::json;

const SID: &str = "abcdef123456";

fn event() -> impl Strategy<Value = Event> {
    let text = "[a-z ]{0,12}";
    prop_oneof![
        text.prop_map(|text| Event::User {
            text,
            kind: "user".into(),
            attachments: None,
            steered: None,
        }),
        (1u64..5, 1u64..9, text).prop_map(|(turn, step, text)| Event::Assistant {
            turn,
            step,
            text
        }),
        (1u64..5, "[a-z]{1,8}").prop_map(|(turn, slot)| Event::TurnStart {
            turn,
            slot,
            model: antiquota_core::DEFAULT_MODEL.into(),
        }),
        (1u64..5, 0u64..1_000, 0u64..100).prop_map(|(turn, input, output)| Event::TurnEnd {
            turn,
            slot: "one@example.com".into(),
            status: "SUCCESS".into(),
            usage: StepUsage {
                input_tokens: input,
                output_tokens: output,
                total_tokens: input + output,
                ..Default::default()
            },
            secs: 1.5,
        }),
        (1u64..5, "[a-z]{1,8}").prop_map(|(turn, line)| Event::Tool {
            turn,
            step: 2,
            name: "run_command".into(),
            params: json!({ "CommandLine": line }),
            state: "done".into(),
            secs: Some(0.25),
            output: Some("ok".into()),
            diff: None,
            error: None,
        }),
        text.prop_map(|text| Event::Notice {
            kind: "info".into(),
            text,
            data: json!({}),
        }),
    ]
}

fn sandbox() -> PathBuf {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).unwrap();
    let root = PathBuf::from(home)
        .join("Desktop/test/antiquota-store-replay")
        .join(uuid::Uuid::new_v4().to_string());
    fs::create_dir_all(&root).unwrap();
    root
}
fn loaded(root: &Path) -> (Session, Vec<EventRecord>) {
    let mut sessions = FileStore::open(root.to_path_buf()).unwrap().load().unwrap();
    assert_eq!(sessions.len(), 1);
    sessions.remove(0)
}
fn expected(records: &[EventRecord]) -> Session {
    let mut session = replay(Session::default(), records).unwrap();
    session.id = SID.into();
    session
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]
    #[test]
    fn a_log_replays_after_restart_and_a_torn_write_loses_only_its_record(
        events in prop::collection::vec(event(), 1..20),
        cut in any::<prop::sample::Index>(),
    ) {
        let root = sandbox();
        let store = FileStore::open(root.clone()).unwrap();
        let meta = Session { id: SID.into(), title: "Replay".into(), ..Default::default() };
        let mut records = vec![EventRecord::new(1.0, Event::Meta { meta: Box::new(meta) })];
        for (at, event) in events.into_iter().enumerate() {
            records.push(EventRecord::new(2.0 + at as f64, event));
        }
        for record in &records {
            store.append(SID, record).unwrap();
        }
        // A new store over the same folder replays exactly what was appended.
        let (session, saved) = loaded(&root);
        prop_assert_eq!(&saved, &records);
        prop_assert_eq!(&session, &expected(&records));
        // The engine stops part way through writing the last record.
        let path = root.join("sessions").join(SID).join("events.jsonl");
        let bytes = fs::read(&path).unwrap();
        let start = bytes[..bytes.len() - 1].iter().rposition(|b| *b == b'\n').unwrap() + 1;
        // Keep less than the whole object, so the torn line can never read as complete.
        let kept = start + cut.index(bytes.len() - 1 - start - 1);
        fs::write(&path, &bytes[..kept]).unwrap();
        let (session, saved) = loaded(&root);
        let whole = &records[..records.len() - 1];
        prop_assert_eq!(&saved[..], whole);
        prop_assert_eq!(&session, &expected(whole));
        // The next append after the restart keeps every earlier record.
        let next = EventRecord::new(1_000.0, Event::Notice {
            kind: "info".into(),
            text: "after the restart".into(),
            data: json!({}),
        });
        FileStore::open(root.clone()).unwrap().append(SID, &next).unwrap();
        let (session, saved) = loaded(&root);
        let mut after = whole.to_vec();
        after.push(next);
        prop_assert_eq!(&saved, &after);
        prop_assert_eq!(&session, &expected(&after));
        let _ = fs::remove_dir_all(&root);
    }
}
