//! Proves application turns, queue control, and replay with only in-memory ports.
mod lifecycle;
use super::*;
use antiquota_core::{
    Attachment, Event, PortFuture, SessionStore, StepUsage, TurnRequest, TurnResult,
};
use std::sync::atomic::{AtomicU64, Ordering};
#[derive(Default)]
struct MemoryStore {
    rows: Mutex<BTreeMap<String, (Session, Vec<EventRecord>)>>,
    usage: Mutex<BTreeMap<(String, u64), StepUsage>>,
}
impl SessionStore for MemoryStore {
    fn load(&self) -> Result<Vec<(Session, Vec<EventRecord>)>, PortError> {
        Ok(self
            .rows
            .lock()
            .unwrap()
            .values()
            .map(|(s, r)| (antiquota_core::replay(s.clone(), r).unwrap(), r.clone()))
            .collect())
    }
    fn append(&self, id: &str, record: &EventRecord) -> Result<(), PortError> {
        self.rows
            .lock()
            .unwrap()
            .entry(id.into())
            .or_default()
            .1
            .push(record.clone());
        Ok(())
    }
    fn save(&self, session: &Session) -> Result<(), PortError> {
        self.rows
            .lock()
            .unwrap()
            .entry(session.id.clone())
            .or_default()
            .0 = session.clone();
        Ok(())
    }
    fn delete(&self, id: &str) -> Result<(), PortError> {
        self.rows.lock().unwrap().remove(id);
        Ok(())
    }
    fn exists(&self, id: &str) -> Result<bool, PortError> {
        Ok(self.rows.lock().unwrap().contains_key(id))
    }
    fn validate_cwd(&self, cwd: &str) -> Result<String, PortError> {
        Ok(cwd.into())
    }
    fn attachments(&self, _id: &str, _paths: &[String]) -> Result<Vec<Attachment>, PortError> {
        Ok(vec![])
    }
    fn turn_timestamp(
        &self,
        _id: &str,
        now: f64,
        records: &[EventRecord],
    ) -> Result<f64, PortError> {
        let mut t = now;
        while records.iter().any(|r| r.ts == t) {
            t = t.next_up();
        }
        Ok(t)
    }
    fn project_usage(&self, id: &str, records: &[EventRecord]) -> Result<(), PortError> {
        let mut rows = self.usage.lock().unwrap();
        for r in records {
            if let Event::TurnEnd { usage, .. } = &r.event {
                rows.insert((id.into(), r.ts.to_bits()), usage.clone());
            }
        }
        Ok(())
    }
    fn state_dir(&self) -> String {
        "memory".into()
    }
}
struct FixedClock;
impl Clock for FixedClock {
    fn now(&self) -> f64 {
        10.0
    }
}
struct Accounts;
struct Lease(antiquota_core::Slot);
impl antiquota_core::AccountLease for Lease {
    fn slot(&self) -> &antiquota_core::Slot {
        &self.0
    }
}
impl AccountStore for Accounts {
    fn accounts(&self) -> Result<Vec<antiquota_core::Account>, PortError> {
        Ok(vec![])
    }
    fn acquire(
        &self,
        _preferred: Option<&str>,
    ) -> Result<Option<Box<dyn antiquota_core::AccountLease>>, PortError> {
        Ok(Some(Box::new(Lease(antiquota_core::Slot {
            name: "fake".into(),
            home: "memory".into(),
        }))))
    }
}
struct Runner {
    calls: AtomicU64,
}
impl AgentRunner for Runner {
    fn run<'a>(
        &'a self,
        request: TurnRequest,
        cancel: Cancellation,
        mut sink: Box<dyn antiquota_core::TurnSink + 'a>,
    ) -> PortFuture<'a, TurnResult> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            if cancel.is_cancelled() {
                return Ok(TurnResult {
                    status: "stopped".into(),
                    ..Default::default()
                });
            }
            sink.event(Event::Delta {
                turn: request.turn,
                step: 1,
                text: request.prompt.clone(),
            })?;
            sink.event(Event::Assistant {
                turn: request.turn,
                step: 1,
                text: request.prompt,
            })?;
            Ok(TurnResult {
                status: "SUCCESS".into(),
                conversation_id: Some("resumable".into()),
                usage: StepUsage {
                    input_tokens: 10,
                    output_tokens: 2,
                    total_tokens: 12,
                    context: 20,
                    ..Default::default()
                },
                error: String::new(),
            })
        })
    }
}
#[tokio::test]
async fn queued_turns_replay_and_delete_retains_usage() {
    let store = Arc::new(MemoryStore::default());
    let runner = Arc::new(Runner {
        calls: AtomicU64::new(0),
    });
    let service = Service::open(
        store.clone(),
        Settings::default(),
        Arc::new(Accounts),
        runner.clone(),
        Arc::new(FixedClock),
    )
    .unwrap();
    let CommandResult::Session(session) = service
        .execute(antiquota_core::Command::Create {
            cwd: "memory".into(),
            title: String::new(),
            model: "model".into(),
            mode: antiquota_core::Mode::Default,
        })
        .await
        .unwrap()
    else {
        panic!("created session")
    };
    assert_eq!(session.id.len(), 12);
    let mut events = service.subscribe();
    assert_eq!(
        service
            .send(&session.id, "same".into(), vec![], "user".into())
            .unwrap(),
        "started"
    );
    assert_eq!(
        service
            .send(&session.id, "same".into(), vec![], "user".into())
            .unwrap(),
        "queued"
    );
    service.wait_idle(&session.id).await;
    let (meta, history) = service.get(&session.id).unwrap();
    assert_eq!(meta.turns, 2);
    let users: Vec<_> = history
        .iter()
        .filter(|record| matches!(record.event, Event::User { .. }))
        .collect();
    assert_eq!(users.len(), 2);
    assert_ne!(users[0].ts, users[1].ts);
    assert_eq!(meta.usage.input, 20);
    assert_eq!(meta.conversation_id.as_deref(), Some("resumable"));
    assert_eq!(store.usage.lock().unwrap().len(), 2);
    assert!(history.iter().all(|r| r.event.is_transcript()));
    let mut deltas = 0;
    while let Ok(event) = events.try_recv() {
        if matches!(event.record.event, Event::Delta { .. }) {
            deltas += 1;
        }
    }
    assert_eq!(deltas, 2);
    let restarted = Service::open(
        store.clone(),
        Settings::default(),
        Arc::new(Accounts),
        runner.clone(),
        Arc::new(FixedClock),
    )
    .unwrap();
    assert_eq!(restarted.get(&session.id).unwrap(), (meta, history));
    assert_eq!(runner.calls.load(Ordering::SeqCst), 2);
    assert!(matches!(
        restarted
            .execute(antiquota_core::Command::Delete {
                session: session.id.clone()
            })
            .await
            .unwrap(),
        CommandResult::Done
    ));
    assert!(restarted.get(&session.id).is_err());
    assert_eq!(store.usage.lock().unwrap().len(), 2);
}
#[tokio::test]
async fn stop_denies_queued_continuation_and_finishes_idle() {
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
        .create("memory", "title", "model", antiquota_core::Mode::Default)
        .unwrap();
    service
        .send(&session.id, "one".into(), vec![], "user".into())
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    service
        .send(&session.id, "two".into(), vec![], "user".into())
        .unwrap();
    service.stop(&session.id).unwrap();
    service.wait_idle(&session.id).await;
    let (meta, history) = service.get(&session.id).unwrap();
    assert_eq!(meta.status, "idle");
    assert!(meta.queue.is_empty());
    assert_eq!(runner.calls.load(Ordering::SeqCst), 1);
    assert!(
        history
            .iter()
            .any(|r| matches!(&r.event,Event::TurnEnd{status,..} if status=="stopped"))
    );
}

struct EmptyAccounts;
impl AccountStore for EmptyAccounts {
    fn accounts(&self) -> Result<Vec<antiquota_core::Account>, PortError> {
        Ok(vec![])
    }
    fn acquire(
        &self,
        _preferred: Option<&str>,
    ) -> Result<Option<Box<dyn antiquota_core::AccountLease>>, PortError> {
        Ok(None)
    }
}
#[tokio::test]
async fn accepted_prompt_is_durable_before_runner_poll_and_survives_no_account_restart() {
    let store = Arc::new(MemoryStore::default());
    let runner = Arc::new(Runner {
        calls: AtomicU64::new(0),
    });
    let service = Service::open(
        store.clone(),
        Settings::default(),
        Arc::new(EmptyAccounts),
        runner.clone(),
        Arc::new(FixedClock),
    )
    .unwrap();
    let CommandResult::Session(session) = service
        .execute(antiquota_core::Command::Create {
            cwd: "memory".into(),
            title: String::new(),
            model: "model".into(),
            mode: antiquota_core::Mode::Default,
        })
        .await
        .unwrap()
    else {
        panic!("session")
    };
    assert!(matches!(
        service
            .execute(antiquota_core::Command::Send {
                session: session.id.clone(),
                text: "Keep this accepted prompt.".into(),
                attachments: vec![],
                kind: "user".into()
            })
            .await
            .unwrap(),
        CommandResult::Sent(_)
    ));
    assert_eq!(runner.calls.load(Ordering::SeqCst), 0);
    let durable = store.load().unwrap();
    assert!(durable[0].1.iter().any(
        |record| matches!(&record.event,Event::User{text,..} if text=="Keep this accepted prompt.")
    ));
    assert_eq!(durable[0].0.status, "waiting");
    service.wait_idle(&session.id).await;
    let restarted = Service::open(
        store,
        Settings::default(),
        Arc::new(EmptyAccounts),
        runner.clone(),
        Arc::new(FixedClock),
    )
    .unwrap();
    let (meta, history) = restarted.get(&session.id).unwrap();
    assert_eq!(meta.status, "idle");
    assert_eq!(runner.calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        history
            .iter()
            .filter(|r| matches!(r.event, Event::User { .. }))
            .count(),
        1
    );
    assert!(history.iter().any(|r|matches!(&r.event,Event::Notice{kind,text,..} if kind=="error"&&text=="no available account")));
}
#[tokio::test]
async fn cancellation_before_runner_start_retains_one_user_event() {
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
        .create("memory", "", "model", antiquota_core::Mode::Default)
        .unwrap();
    service
        .send(
            &session.id,
            "Cancel before launch.".into(),
            vec![],
            "user".into(),
        )
        .unwrap();
    service.stop(&session.id).unwrap();
    service.wait_idle(&session.id).await;
    let (meta, history) = service.get(&session.id).unwrap();
    assert_eq!(meta.status, "idle");
    assert_eq!(runner.calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        history
            .iter()
            .filter(|r| matches!(r.event, Event::User { .. }))
            .count(),
        1
    );
}
