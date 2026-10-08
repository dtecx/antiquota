//! Proves conversation actions without adapters or an agent call.
use super::*;
use serde_json::json;
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
