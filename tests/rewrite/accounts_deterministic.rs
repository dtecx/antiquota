//! Proves timed handoff using real process and file adapters with an injected clock.
use crate::{
    setup,
    support::{self, Server},
};
use antiquota_accounts::FileAccounts;
use antiquota_agy::AgyRunner;
use antiquota_app::Service;
use antiquota_core::{Cancellation, Clock, Command, Event, Mode, PortFuture, SessionStore};
use antiquota_store::FileStore;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    process::Command as Process,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const WORKER: &str = "ANTIQUOTA_SLICE2_TEST_WORKER";
const NOW: f64 = 1_800_000_000.0;

/// Advances policy time instantly, or holds one cancellable wait for Stop proof.
struct TestClock {
    now: Mutex<f64>,
    waits: Mutex<Vec<f64>>,
    controlled: bool,
}
impl TestClock {
    fn new(controlled: bool) -> Arc<Self> {
        Arc::new(Self {
            now: Mutex::new(NOW),
            waits: Mutex::new(vec![]),
            controlled,
        })
    }
    fn waits(&self) -> Vec<f64> {
        self.waits.lock().unwrap().clone()
    }
}
impl Clock for TestClock {
    fn now(&self) -> f64 {
        *self.now.lock().unwrap()
    }
    fn wait(&self, seconds: f64, cancel: Cancellation) -> PortFuture<'_, ()> {
        Box::pin(async move {
            self.waits.lock().unwrap().push(seconds);
            if self.controlled {
                while !cancel.is_cancelled() {
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
            } else {
                *self.now.lock().unwrap() += seconds;
                tokio::task::yield_now().await;
            }
            Ok(())
        })
    }
}

/// Re-executes the test with isolated child environment instead of mutating global environment.
fn worker(name: &str, script: Value) -> bool {
    if std::env::var(WORKER).as_deref() == Ok(name) {
        return true;
    }
    let server = Server::new_stopped(name, true);
    setup::script(&server, script);
    let output = support::bounded_output(
        Process::new(std::env::current_exe().unwrap())
            .args(["--exact", &format!("deterministic::{name}"), "--nocapture"])
            .current_dir(&server.project)
            .env(WORKER, name)
            .env("ANTIQUOTA_HOME", &server.home)
            .env("ANTIQUOTA_DIR", &server.data)
            .env("ANTIQUOTA_AGY", &server.agy)
            .env("ANTIQUOTA_NO_USAGE_LINK", "1")
            .env("TOKIO_WORKER_THREADS", "2")
            .env("FAKE_AGY_SCRIPT", &server.script)
            .env("FAKE_AGY_RECORD", server.root.join("fake-calls.jsonl"))
            .env_remove("FAKE_AGY_SCENARIO"),
    );
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
    false
}

fn compose(clock: Arc<TestClock>) -> Arc<Service> {
    let path = |name| PathBuf::from(std::env::var_os(name).unwrap());
    let store = Arc::new(FileStore::open(path("ANTIQUOTA_DIR")).unwrap());
    let accounts = Arc::new(FileAccounts::with_runtime(
        path("ANTIQUOTA_DIR"),
        path("ANTIQUOTA_HOME"),
        path("ANTIQUOTA_AGY"),
    ));
    let settings = store.settings().unwrap().unwrap_or_default();
    let runner = Arc::new(AgyRunner {
        executable: path("ANTIQUOTA_AGY"),
        hook_program: support::binary("antiquota"),
        isolated_fake: true,
    });
    Service::open_with_quota(
        store,
        settings,
        accounts.clone(),
        runner,
        clock,
        Some(accounts),
    )
    .unwrap()
}

fn calls() -> Vec<Value> {
    std::fs::read_to_string(std::env::var_os("FAKE_AGY_RECORD").unwrap())
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter(|row| {
            row["argv"]
                .as_array()
                .unwrap()
                .windows(2)
                .any(|args| args[0] == "--output-format" && args[1] == "stream-json")
        })
        .collect()
}

fn send(service: &Arc<Service>, prompt: &str) -> String {
    let session = service
        .create(
            std::env::current_dir().unwrap().to_str().unwrap(),
            "Deterministic handoff",
            "gemini-3.8-flash-high",
            Mode::Default,
        )
        .unwrap();
    assert_eq!(
        service
            .send(&session.id, prompt.into(), vec![], "user".into())
            .unwrap(),
        "started"
    );
    session.id
}
async fn idle(service: &Arc<Service>) {
    let started = Instant::now();
    while service.busy_count() != 0 {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "timed handoff did not finish"
        );
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
}

#[tokio::test]
async fn transient_retries_use_fake_time_then_park_and_resume_on_next_account() {
    let name = "transient_retries_use_fake_time_then_park_and_resume_on_next_account";
    if !worker(name, json!({"scenarios_by_slot":{setup::ONE:"transient"}})) {
        return;
    }
    let clock = TestClock::new(false);
    let service = compose(clock.clone());
    let id = send(&service, "Retry this request once through policy.");
    idle(&service).await;
    let rows = calls();
    assert_eq!(rows.len(), 4);
    assert!(rows[..3].iter().all(|row| row["slot"] == setup::ONE));
    assert_eq!(rows[3]["slot"], setup::TWO);
    assert_eq!(clock.waits(), [10.0, 30.0]);
    let (session, events) = service.get(&id).unwrap();
    assert_eq!(session.last_slot.as_deref(), Some(setup::TWO));
    assert_eq!(session.usage.input, 100);
    assert_eq!(
        events
            .iter()
            .filter(|row| matches!(row.event, Event::User { .. }))
            .count(),
        1
    );
    assert_eq!(
        setup::argument(&rows[0], "-p"),
        Some(
            format!(
                "{}\n\nRetry this request once through policy.",
                antiquota_core::jobs::JOBS_HINT
            )
            .as_str()
        )
    );
    for row in &rows[1..] {
        assert_eq!(
            setup::argument(row, "--conversation"),
            session.conversation_id.as_deref()
        );
        assert_ne!(setup::argument(row, "-p"), setup::argument(&rows[0], "-p"));
    }
    let accounts = service.account_overview().unwrap();
    assert_eq!(accounts[0].walls["gemini"], NOW + 40.0 + 300.0);
    assert_eq!(accounts[0].why["gemini"], "repeated errors");
    setup::evidence(
        "deterministic-retry",
        json!({"calls":4,"waits":[10,30],"cooldown":300,"sameConversation":true,"userEvents":1,"currentInput":100}),
    );
    service.shutdown().await;
}

#[tokio::test]
async fn repeated_transient_failures_stop_after_bounded_account_attempts() {
    let name = "repeated_transient_failures_stop_after_bounded_account_attempts";
    if !worker(name, json!({"scenario":"transient"})) {
        return;
    }
    let clock = TestClock::new(false);
    let service = compose(clock.clone());
    let id = send(&service, "Both accounts fail transiently.");
    idle(&service).await;
    assert_eq!(calls().len(), 6);
    assert_eq!(clock.waits(), [10.0, 30.0, 10.0, 30.0]);
    let (session, events) = service.get(&id).unwrap();
    assert_eq!(session.status, "idle");
    assert!(session.queue.is_empty());
    assert_eq!(session.usage.input, 0);
    assert_eq!(
        events
            .iter()
            .filter(|row| matches!(row.event, Event::User { .. }))
            .count(),
        1
    );
    assert!(
        service
            .account_overview()
            .unwrap()
            .iter()
            .all(|account| account.walls["gemini"] > clock.now())
    );
    setup::evidence(
        "bounded-retries",
        json!({"calls":6,"waits":[10,30,10,30],"idle":true,"userEvents":1,"queueEmpty":true}),
    );
    service.shutdown().await;
}

#[tokio::test]
async fn stop_cancels_fake_clock_retry_without_false_wall_or_extra_runner() {
    let name = "stop_cancels_fake_clock_retry_without_false_wall_or_extra_runner";
    if !worker(name, json!({"scenario":"transient"})) {
        return;
    }
    let clock = TestClock::new(true);
    let service = compose(clock.clone());
    let id = send(&service, "Stop during retry policy wait.");
    let started = Instant::now();
    while clock.waits().is_empty() {
        assert!(started.elapsed() < Duration::from_secs(3));
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    assert_eq!(clock.waits(), [10.0]);
    service
        .execute(Command::Stop {
            session: id.clone(),
        })
        .await
        .unwrap();
    idle(&service).await;
    assert_eq!(calls().len(), 1);
    let (session, _) = service.get(&id).unwrap();
    assert!(session.conversation_id.is_some());
    assert!(session.queue.is_empty());
    assert!(
        service
            .account_overview()
            .unwrap()
            .iter()
            .all(|account| account.walls["gemini"] == 0.0 && !account.busy)
    );
    service.shutdown().await;
    let recovered = compose(TestClock::new(false));
    assert_eq!(
        recovered.get(&id).unwrap().0.conversation_id,
        session.conversation_id
    );
    setup::evidence(
        "retry-cancel",
        json!({"calls":1,"cancelledWait":10,"falseWalls":0,"conversationPersists":true}),
    );
    recovered.shutdown().await;
}

#[tokio::test]
async fn warm_account_affinity_waits_ten_fake_seconds_before_another_lease() {
    let name = "warm_account_affinity_waits_ten_fake_seconds_before_another_lease";
    if !worker(name, json!({})) {
        return;
    }
    let clock = TestClock::new(false);
    let service = compose(clock.clone());
    let id = send(&service, "Create warm account affinity.");
    idle(&service).await;
    let conversation = service.get(&id).unwrap().0.conversation_id;
    let lock = PathBuf::from(std::env::var_os("ANTIQUOTA_DIR").unwrap())
        .join("state/leases/one@example.com.lock");
    let lease = antiquota_store::lock_file(&lock).unwrap().unwrap();
    service
        .send(
            &id,
            "Use another account when affinity wait expires.".into(),
            vec![],
            "user".into(),
        )
        .unwrap();
    idle(&service).await;
    assert_eq!(clock.waits(), vec![1.0; 10]);
    let rows = calls();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["slot"], setup::ONE);
    assert_eq!(rows[1]["slot"], setup::TWO);
    assert_eq!(
        setup::argument(&rows[1], "--conversation"),
        conversation.as_deref()
    );
    assert!(
        service
            .accounts
            .routes(100)
            .unwrap()
            .iter()
            .any(|route| route.event == "affinity-broke"
                && route.previous.as_deref() == Some(setup::ONE))
    );
    drop(lease);
    setup::evidence(
        "affinity-wait",
        json!({"fakeSeconds":10,"waits":[1,1,1,1,1,1,1,1,1,1],"nextSlot":setup::TWO,"sameConversation":true}),
    );
    service.shutdown().await;
}
