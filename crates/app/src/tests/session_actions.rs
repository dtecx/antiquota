//! Proves conversation actions without adapters or an agent call.
use super::*;
use serde_json::json;

fn notice_service() -> (Arc<Service>, Arc<MemoryStore>, String) {
    let store = Arc::new(MemoryStore::default());
    let service = Service::open(
        store.clone(),
        Settings::default(),
        Arc::new(Accounts),
        Arc::new(Runner {
            calls: AtomicU64::new(0),
        }),
        Arc::new(FixedClock),
    )
    .unwrap();
    let session = service
        .create("memory", "Notices", "model", antiquota_core::Mode::Default)
        .unwrap();
    (service, store, session.id)
}

#[test]
fn current_denial_suggestions_are_vetted_before_persistence_and_broadcast() {
    let (service, store, id) = notice_service();
    let mut receiver = service.subscribe();
    let data = json!({
        "commands": ["git status", "git status", "/usr/bin/git status --short", "/bin/bash -c unsafe", "/usr/bin/python3.11 -c unsafe", "git push --force origin main", "/usr/bin/find . -delete", false, null],
        "rules": ["command(*)"], "actions": [], "future": {"preserved": true}
    });
    let mut expected = data.clone();
    expected["rules"] = json!(["command(git status)", "command(/usr/bin/git status)"]);
    service
        .notice(&id, "denied", "Denied commands.", data)
        .unwrap();
    let event = receiver.try_recv().unwrap();
    assert_eq!(event.sid.as_deref(), Some(id.as_str()));
    assert!(
        matches!(&event.record.event, Event::Notice { kind, data, .. } if kind == "denied" && data == &expected)
    );
    let rows = store.rows.lock().unwrap();
    assert!(
        matches!(&rows[&id].1.last().unwrap().event, Event::Notice { data, .. } if data == &expected)
    );
    drop(rows);
    assert!(
        matches!(&service.get(&id).unwrap().1.last().unwrap().event, Event::Notice { data, .. } if data == &expected)
    );
}

#[test]
fn historical_denial_views_recompute_rules_without_rewriting_stored_records() {
    let (service, store, id) = notice_service();
    let data = json!({
        "commands": ["git status", "'/bin/bash' -c unsafe", "/usr/bin/git reset --hard HEAD"],
        "rules": ["command(*)", "command(bash)"],
        "actions": [{"action":"write_to_file", "display_name":"Historical edit"}],
        "future": ["preserved"]
    });
    let cell = service.cell(&id).unwrap();
    service
        .append(
            &mut cell.state.lock().unwrap(),
            Event::Notice {
                kind: "denied".into(),
                text: "Historical denial.".into(),
                data: data.clone(),
            },
        )
        .unwrap();
    let original = serde_json::to_vec(&store.rows.lock().unwrap()[&id].1).unwrap();
    for _ in 0..2 {
        let (_, records) = service.get(&id).unwrap();
        let Event::Notice { data: view, .. } = &records.last().unwrap().event else {
            panic!("notice missing")
        };
        let mut expected = data.clone();
        expected["rules"] = json!(["command(git status)"]);
        assert_eq!(view, &expected);
        assert_eq!(
            serde_json::to_vec(&store.rows.lock().unwrap()[&id].1).unwrap(),
            original
        );
    }
    let reopened = Service::open(
        store.clone(),
        Settings::default(),
        Arc::new(Accounts),
        Arc::new(Runner {
            calls: AtomicU64::new(0),
        }),
        Arc::new(FixedClock),
    )
    .unwrap();
    assert!(
        matches!(&reopened.get(&id).unwrap().1.last().unwrap().event, Event::Notice { data, .. } if data["rules"] == json!(["command(git status)"]) && data["future"] == json!(["preserved"]))
    );
    assert_eq!(
        serde_json::to_vec(&store.rows.lock().unwrap()[&id].1).unwrap(),
        original
    );
}

#[test]
fn a_failed_known_write_tool_emits_a_command_free_denial_notice() {
    let (service, _, id) = notice_service();
    let cell = service.cell(&id).unwrap();
    service.append(&mut cell.state.lock().unwrap(), Event::Tool {
        turn: 1, step: 1, name: "replace_file_content".into(),
        params: json!({"TargetFile":"memory/file.rs"}), state: "ERROR".into(),
        secs: Some(0.1), output: None, diff: None,
        error: Some(json!({"message":"permission check failed: user denied permission to edit file"})),
    }).unwrap();
    let mut receiver = service.subscribe();
    let outcome = service.turn_outcome(&id, 1, true, "").unwrap();
    assert!(outcome.denied.is_empty());
    let event = receiver.try_recv().unwrap();
    let expected = json!({"commands":[],"actions":[{"action":"replace_file_content","display_name":"replace_file_content"}],"rules":[]});
    assert!(
        matches!(&event.record.event, Event::Notice { kind, data, .. } if kind == "denied" && data == &expected)
    );
    assert!(
        matches!(&service.get(&id).unwrap().1.last().unwrap().event, Event::Notice { data, .. } if data == &expected)
    );
}

#[tokio::test]
async fn fork_inherits_handoff_without_jobs_goal_or_worktree_and_clear_keeps_history() {
    let store = Arc::new(MemoryStore::default());
    let runner = Arc::new(Runner {
        calls: AtomicU64::new(0),
    });
    let service = Service::open(
        store,
        Settings::default(),
        Arc::new(Accounts),
        runner.clone(),
        Arc::new(FixedClock),
    )
    .unwrap();
    let session = service
        .create("memory", "Original", "model", antiquota_core::Mode::Plan)
        .unwrap();
    let cell = service.cell(&session.id).unwrap();
    {
        let mut state = cell.state.lock().unwrap();
        let mut next = state.meta.clone();
        next.effort = "high".into();
        next.jobs = vec![json!({"cmd":"job"})];
        next.goal = Some(serde_json::from_value(json!({"objective":"Goal"})).unwrap());
        next.worktree = Some(json!({"branch":"owned"}));
        next.conversation_id = Some("old".into());
        next.context = 100;
        service.replace_meta(&mut state, next).unwrap();
        service
            .append(
                &mut state,
                Event::User {
                    steered: None,
                    text: "Earlier request".into(),
                    kind: "user".into(),
                    attachments: None,
                },
            )
            .unwrap();
    }
    let fork = service.fork(&session.id).unwrap();
    assert_eq!(fork.title, "Original (fork)");
    assert_eq!(fork.cwd, session.cwd);
    assert_eq!(fork.mode, session.mode);
    assert_eq!(fork.effort, "high");
    assert!(fork.jobs.is_empty());
    assert!(fork.goal.is_none());
    assert!(fork.worktree.is_none());
    assert!(fork.conversation_id.is_none());
    assert!(fork.pending_note.contains("Earlier request"));
    service.inherit(&fork.id, &session.id).unwrap();
    assert_eq!(
        service
            .get(&fork.id)
            .unwrap()
            .0
            .pending_note
            .matches("# Handoff: Original")
            .count(),
        2
    );
    let before = service.get(&session.id).unwrap().1;
    service.clear(&session.id, true).unwrap();
    let (compact, events) = service.get(&session.id).unwrap();
    assert!(compact.pending_note.contains("Earlier request"));
    assert!(compact.conversation_id.is_none());
    assert_eq!(compact.context, 0);
    assert_eq!(&events[..before.len()], before.as_slice());
    service.clear(&session.id, false).unwrap();
    assert!(service.get(&session.id).unwrap().0.pending_note.is_empty());
    assert_eq!(runner.calls.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn search_returns_forty_newest_sessions_and_ignores_non_message_events() {
    let service = Service::open(
        Arc::new(MemoryStore::default()),
        Settings::default(),
        Arc::new(Accounts),
        Arc::new(Runner {
            calls: AtomicU64::new(0),
        }),
        Arc::new(FixedClock),
    )
    .unwrap();
    for n in 0..45 {
        let meta = service
            .create(
                "memory",
                &format!("Session {n}"),
                "model",
                antiquota_core::Mode::Default,
            )
            .unwrap();
        let cell = service.cell(&meta.id).unwrap();
        let mut state = cell.state.lock().unwrap();
        service
            .append(
                &mut state,
                Event::User {
                    steered: None,
                    text: "old needle".into(),
                    kind: "user".into(),
                    attachments: None,
                },
            )
            .unwrap();
        service
            .append(
                &mut state,
                Event::Assistant {
                    turn: 1,
                    step: 1,
                    text: format!("{}NEEDLE{}", "界".repeat(70), "界".repeat(200)),
                },
            )
            .unwrap();
        state.records.last_mut().unwrap().ts = n as f64;
        service
            .append(
                &mut state,
                Event::Notice {
                    kind: "info".into(),
                    text: "needle notice".into(),
                    data: json!({}),
                },
            )
            .unwrap();
    }
    assert_eq!(service.search(" n ").unwrap(), json!([]));
    let hits = service.search(" NEEDLE ").unwrap();
    let hits = hits.as_array().unwrap();
    assert_eq!(hits.len(), 40);
    assert_eq!(hits[0]["ts"], 44.0);
    assert_eq!(hits[39]["ts"], 5.0);
    assert!(hits.iter().all(|h| h["who"] == "assistant"
        && h["snippet"].as_str().unwrap().starts_with("…")
        && h["snippet"].as_str().unwrap().chars().count() == 161));
}
