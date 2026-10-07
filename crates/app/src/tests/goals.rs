//! Proves goal sleeping on spent quota and verifier cancellation with in-memory ports.
use super::*;
use antiquota_core::{Account, AskRequest, AskResult, Goal, Slot};
use serde_json::json;
use std::sync::atomic::AtomicBool;

/// Advances time instantly on every wait and records how long each wait was.
struct SkippingClock {
    now: Mutex<f64>,
    waits: Mutex<Vec<f64>>,
}
impl Clock for SkippingClock {
    fn now(&self) -> f64 {
        *self.now.lock().unwrap()
    }
    fn wait(&self, seconds: f64, _cancel: Cancellation) -> PortFuture<'_, ()> {
        Box::pin(async move {
            self.waits.lock().unwrap().push(seconds);
            *self.now.lock().unwrap() += seconds;
            tokio::task::yield_now().await;
            Ok(())
        })
    }
    fn local_hhmm(&self, instant: f64) -> String {
        format!("@{instant}")
    }
}

/// One account whose Gemini pool is walled until `until`.
struct WalledAccounts {
    until: f64,
}
impl AccountStore for WalledAccounts {
    fn accounts(&self) -> Result<Vec<Account>, PortError> {
        self.accounts_at(0.0)
    }
    fn accounts_at(&self, _now: f64) -> Result<Vec<Account>, PortError> {
        Ok(vec![Account {
            slot: "fake".into(),
            walls: [("gemini".to_owned(), self.until)].into(),
            ..Default::default()
        }])
    }
    fn record_route(&self, _entry: &antiquota_core::RoutingEntry) -> Result<(), PortError> {
        Ok(())
    }
    fn acquire(
        &self,
        _preferred: Option<&str>,
    ) -> Result<Option<Box<dyn antiquota_core::AccountLease>>, PortError> {
        Ok(Some(Box::new(Lease(Slot {
            name: "fake".into(),
            home: "memory".into(),
        }))))
    }
}

#[tokio::test]
async fn a_goal_sleeps_until_the_wall_lifts_then_continues() {
    let clock = Arc::new(SkippingClock {
        now: Mutex::new(1_000.0),
        waits: Mutex::new(vec![]),
    });
    let service = Service::open(
        Arc::new(MemoryStore::default()),
        Settings::default(),
        Arc::new(WalledAccounts { until: 1_600.0 }),
        Arc::new(Runner {
            calls: AtomicU64::new(0),
        }),
        clock.clone(),
    )
    .unwrap();
    let id = service
        .create("memory", "", "model", antiquota_core::Mode::Default)
        .unwrap()
        .id;
    service
        .set_goal(&id, "Finish.", "", false, "", 3, true)
        .unwrap();
    service.wait_idle(&id).await;
    let (meta, events) = service.get(&id).unwrap();
    assert_eq!(
        meta.goal.unwrap().state,
        "met",
        "the reply claims completion"
    );
    assert_eq!(
        clock.waits.lock().unwrap()[0],
        630.0,
        "until reset plus 30 seconds"
    );
    let sleeping = events.iter().find_map(|r| match &r.event {
        Event::Notice { kind, text, data } if kind == "wall" => Some((text.clone(), data.clone())),
        _ => None,
    });
    assert_eq!(
        sleeping,
        Some((
            "Every account is out of Gemini quota. Sleeping until @1600, then the goal continues."
                .into(),
            json!({"reset": 1600.0})
        ))
    );
    let statuses: Vec<String> = service
        .store
        .load()
        .unwrap()
        .into_iter()
        .flat_map(|(_, records)| records)
        .filter_map(|r| match r.event {
            Event::Meta { meta } => Some(meta.status.clone()),
            _ => None,
        })
        .collect();
    assert!(statuses.iter().any(|s| s == "sleeping"));
}

#[tokio::test]
async fn without_a_goal_spent_quota_ends_the_turn() {
    let clock = Arc::new(SkippingClock {
        now: Mutex::new(1_000.0),
        waits: Mutex::new(vec![]),
    });
    let service = Service::open(
        Arc::new(MemoryStore::default()),
        Settings::default(),
        Arc::new(WalledAccounts { until: 1_600.0 }),
        Arc::new(Runner {
            calls: AtomicU64::new(0),
        }),
        clock.clone(),
    )
    .unwrap();
    let id = service
        .create("memory", "", "model", antiquota_core::Mode::Default)
        .unwrap()
        .id;
    service
        .send(&id, "hi".into(), vec![], "user".into())
        .unwrap();
    service.wait_idle(&id).await;
    assert!(clock.waits.lock().unwrap().is_empty());
    let (_, events) = service.get(&id).unwrap();
    assert!(events.iter().any(|r| matches!(&r.event, Event::Notice { kind, text, .. }
        if kind == "wall" && text.starts_with("Every account is out of Gemini quota. Send again later"))));
}

/// Claims completion, then answers the verifier only after it is cancelled.
struct HangingVerifier {
    asked: AtomicBool,
}
impl AgentRunner for HangingVerifier {
    fn run<'a>(
        &'a self,
        request: TurnRequest,
        _cancel: Cancellation,
        mut sink: Box<dyn antiquota_core::TurnSink + 'a>,
    ) -> PortFuture<'a, TurnResult> {
        Box::pin(async move {
            sink.event(Event::Assistant {
                turn: request.turn,
                step: 1,
                text: "Done.\nGOAL_COMPLETE".into(),
            })?;
            Ok(TurnResult {
                status: "SUCCESS".into(),
                ..Default::default()
            })
        })
    }
    fn ask<'a>(&'a self, _request: AskRequest, cancel: Cancellation) -> PortFuture<'a, AskResult> {
        Box::pin(async move {
            self.asked.store(true, Ordering::SeqCst);
            while !cancel.is_cancelled() {
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
            Ok(AskResult {
                status: "stopped".into(),
                ..Default::default()
            })
        })
    }
}

#[tokio::test]
async fn stopping_during_the_verifier_never_marks_the_goal_met() {
    let runner = Arc::new(HangingVerifier {
        asked: AtomicBool::new(false),
    });
    let service = Service::open(
        Arc::new(MemoryStore::default()),
        Settings::default(),
        Arc::new(Accounts),
        runner.clone(),
        Arc::new(FixedClock),
    )
    .unwrap();
    let id = service
        .create("memory", "", "model", antiquota_core::Mode::Default)
        .unwrap()
        .id;
    service
        .set_goal(&id, "Finish.", "", true, "", 3, true)
        .unwrap();
    while !runner.asked.load(Ordering::SeqCst) {
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    assert_eq!(service.get(&id).unwrap().0.status, "verifying");
    service.stop(&id).unwrap();
    service.wait_idle(&id).await;
    let (meta, events) = service.get(&id).unwrap();
    assert_eq!(
        meta.goal,
        Some(Goal {
            objective: "Finish.".into(),
            verifier: true,
            max_rounds: 3,
            round: 1,
            state: "paused".into(),
            ..Default::default()
        })
    );
    assert_eq!(meta.status, "idle");
    assert!(
        !events
            .iter()
            .any(|r| matches!(&r.event, Event::Notice { kind, .. } if kind == "verdict"))
    );
}

/// A check command that prints a lot, or never finishes.
struct ScriptedShell {
    timed_out: bool,
}
impl antiquota_core::ShellRunner for ScriptedShell {
    fn run<'a>(
        &'a self,
        _cwd: &'a str,
        command: &'a str,
        timeout: f64,
        _cancel: Cancellation,
    ) -> PortFuture<'a, antiquota_core::CommandOutcome> {
        Box::pin(async move {
            assert_eq!(timeout, 1800.0);
            assert_eq!(command, "make check");
            Ok(antiquota_core::CommandOutcome {
                code: (!self.timed_out).then_some(1),
                output: format!("{}END", "x".repeat(5000)),
                timed_out: self.timed_out,
            })
        })
    }
}

#[tokio::test]
async fn check_results_keep_the_last_output_and_report_timeouts() {
    for timed_out in [false, true] {
        let service = Service::compose(crate::Ports {
            store: Arc::new(MemoryStore::default()),
            settings: Settings::default(),
            accounts: Arc::new(Accounts),
            runner: Arc::new(HangingVerifier {
                asked: AtomicBool::new(false),
            }),
            clock: Arc::new(FixedClock),
            quota: None,
            shell: Some(Arc::new(ScriptedShell { timed_out })),
        })
        .unwrap();
        let id = service
            .create("memory", "", "model", antiquota_core::Mode::Default)
            .unwrap()
            .id;
        service
            .set_goal(&id, "Pass.", "make check", true, "", 1, true)
            .unwrap();
        service.wait_idle(&id).await;
        let (meta, events) = service.get(&id).unwrap();
        assert_eq!(meta.goal.unwrap().state, "failed");
        let check = events
            .iter()
            .find_map(|r| match &r.event {
                Event::Notice { kind, text, data } if kind == "check" => {
                    Some((text.clone(), data.clone()))
                }
                _ => None,
            })
            .unwrap();
        let output = check.1["output"].as_str().unwrap();
        assert_eq!(output.chars().count(), 3000);
        assert!(output.ends_with("END"));
        assert_eq!(check.1["passed"], false);
        if timed_out {
            assert_eq!(check.0, "Check `make check` exited timeout.");
            assert_eq!(check.1["rc"], "timeout");
        } else {
            assert_eq!(check.1["rc"], 1);
        }
    }
}
