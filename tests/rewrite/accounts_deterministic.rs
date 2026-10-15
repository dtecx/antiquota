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
    // Composite workers can perform three separately bounded 20-second turns.
    let output = support::bounded_output_with_timeout(
        Process::new(std::env::current_exe().unwrap())
            .args(["--exact", &format!("deterministic::{name}"), "--nocapture"])
            .current_dir(&server.project)
            .env(WORKER, name)
            .env("ANTIQUOTA_HOME", &server.home)
            .env("ANTIQUOTA_DIR", &server.data)
            .env("ANTIQUOTA_AGY", &server.agy)
            .env("ANTIQUOTA_NO_USAGE_LINK", "1")
            .env("ANTIQUOTA_SIGNIN_PROBE", "online")
            .env("TOKIO_WORKER_THREADS", "2")
            .env("FAKE_AGY_SCRIPT", &server.script)
            .env("FAKE_AGY_RECORD", server.root.join("fake-calls.jsonl"))
            .env_remove("FAKE_AGY_SCENARIO"),
        Duration::from_secs(90),
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

fn all_calls() -> Vec<Value> {
    std::fs::read_to_string(std::env::var_os("FAKE_AGY_RECORD").unwrap())
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect()
}
fn routing_rows() -> Vec<Value> {
    std::fs::read_to_string(
        PathBuf::from(std::env::var_os("ANTIQUOTA_DIR").unwrap()).join("state/routing.jsonl"),
    )
    .unwrap_or_default()
    .lines()
    .map(|line| serde_json::from_str::<Value>(line).unwrap())
    .collect()
}

#[tokio::test]
async fn routing_ledger_stamps_policy_time_and_auth_parks_exactly_thirty_days() {
    let name = "routing_ledger_stamps_policy_time_and_auth_parks_exactly_thirty_days";
    if !worker(
        name,
        json!({"scenarios_by_slot":{setup::ONE:"transient",setup::TWO:"auth"}}),
    ) {
        return;
    }
    let clock = TestClock::new(false);
    let service = compose(clock.clone());
    let id = send(&service, "Record every routing decision.");
    idle(&service).await;
    assert_eq!(clock.waits(), [10.0, 30.0]);
    let parked = NOW + 40.0 + 30.0 * 86_400.0;
    let accounts = service.account_overview().unwrap();
    assert_eq!(accounts[0].walls["gemini"], NOW + 40.0 + 300.0);
    assert_eq!(accounts[1].walls["gemini"], parked);
    assert_eq!(accounts[1].walls["cg"], parked);
    assert_eq!(accounts[1].why["cg"], "signed out: sign in again");
    service
        .account_service
        .pause_account(setup::ONE, true)
        .unwrap();
    service
        .account_service
        .pause_account(setup::ONE, false)
        .unwrap();
    let error = "UNAVAILABLE: the model is overloaded";
    assert_eq!(
        routing_rows(),
        [
            json!({"ts":NOW,"event":"pick","slot":setup::ONE,"bucket":"gemini","session":id,"previous":null}),
            json!({"ts":NOW,"event":"retry","slot":setup::ONE,"bucket":"gemini","session":id,"why":error,"wait":10.0}),
            json!({"ts":NOW + 10.0,"event":"retry","slot":setup::ONE,"bucket":"gemini","session":id,"why":error,"wait":30.0}),
            json!({"ts":NOW + 40.0,"event":"cooldown","slot":setup::ONE,"bucket":"gemini","why":"repeated errors","secs":300.0}),
            json!({"ts":NOW + 40.0,"event":"pick","slot":setup::TWO,"bucket":"gemini","session":id,"previous":setup::ONE}),
            json!({"ts":NOW + 40.0,"event":"signed-out","slot":setup::TWO,"why":"agy rejected the account's token"}),
            json!({"ts":NOW + 40.0,"event":"paused","slot":setup::ONE,"why":"by the user"}),
            json!({"ts":NOW + 40.0,"event":"resumed","slot":setup::ONE,"why":"by the user"}),
        ]
    );
    // A healthy reading repairs the signed-out account.
    service
        .account_service
        .check_account(setup::TWO)
        .await
        .unwrap();
    let repaired = &service.account_overview().unwrap()[1];
    assert_eq!(repaired.walls["gemini"], 0.0);
    assert_eq!(repaired.walls["cg"], 0.0);
    assert_eq!(repaired.probed, Some(NOW + 40.0));
    setup::evidence(
        "routing-timestamps",
        json!({"events":["pick","retry","retry","cooldown","pick","signed-out","paused","resumed"],"parkedSeconds":2592000,"repairedByProbe":true}),
    );
    service.shutdown().await;
}

#[tokio::test]
async fn quota_wall_ledger_entry_and_verification_parking_use_policy_time() {
    let name = "quota_wall_ledger_entry_and_verification_parking_use_policy_time";
    if !worker(
        name,
        json!({"scenarios_by_slot":{setup::ONE:"quota"},"metadata_by_slot":{
            setup::ONE:{"quota_after_turn":setup::healthy_quota(0.0)},
            setup::TWO:{"verify_url":"https://accounts.google.com/signin/continue?fixture=1"}
        }}),
    ) {
        return;
    }
    let clock = TestClock::new(false);
    let service = compose(clock.clone());
    service
        .account_service
        .check_account(setup::TWO)
        .await
        .unwrap();
    let parked = NOW + 30.0 * 86_400.0;
    let verify = &service.account_overview().unwrap()[1];
    assert_eq!(verify.walls["gemini"], parked);
    assert_eq!(verify.walls["cg"], parked);
    assert_eq!(verify.why["gemini"], "verify the account with Google");
    let id = send(&service, "Spend the first account.");
    idle(&service).await;
    let wall = 4_070_926_800.0 + 120.0;
    assert_eq!(service.account_overview().unwrap()[0].walls["gemini"], wall);
    assert_eq!(
        routing_rows(),
        [
            json!({"ts":NOW,"event":"pick","slot":setup::ONE,"bucket":"gemini","session":id,"previous":null}),
            json!({"ts":NOW,"event":"wall","slot":setup::ONE,"bucket":"gemini","until":wall,"why":"quota spent"}),
        ]
    );
    setup::evidence(
        "wall-timestamps",
        json!({"verificationParkedSeconds":2592000,"wallEntry":"quota spent","until":wall}),
    );
    service.shutdown().await;
}

#[tokio::test]
async fn polling_uses_quota_poll_freshness_and_refreshes_only_stale_idle_accounts() {
    let name = "polling_uses_quota_poll_freshness_and_refreshes_only_stale_idle_accounts";
    if !worker(name, json!({})) {
        return;
    }
    let data = PathBuf::from(std::env::var_os("ANTIQUOTA_DIR").unwrap());
    let stamp = |slot: &str, at: f64| {
        let path = data.join(format!("state/quota/{slot}.json"));
        let mut value: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        value["at"] = json!(at);
        std::fs::write(path, value.to_string()).unwrap();
    };
    let probes = || {
        all_calls()
            .into_iter()
            .filter(|row| setup::argument(row, "-p") == Some("/quota"))
            .map(|row| row["slot"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>()
    };
    for (poll, freshness) in [("abc", 600.0), ("1", 120.0), ("5", 300.0)] {
        std::fs::write(
            data.join("settings.json"),
            json!({"quota_poll":poll,"keep_awake":"off"}).to_string(),
        )
        .unwrap();
        // One account is a second younger than the freshness age, the other exactly at it.
        stamp(setup::ONE, NOW - freshness + 1.0);
        stamp(setup::TWO, NOW - freshness);
        let before = probes().len();
        let clock = TestClock::new(true);
        let service = compose(clock.clone());
        service.account_service.start_account_polling();
        let started = Instant::now();
        while clock.waits().is_empty() {
            assert!(started.elapsed() < Duration::from_secs(5), "{poll}");
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        assert_eq!(clock.waits(), [freshness], "{poll}");
        assert_eq!(probes()[before..], [setup::TWO.to_owned()], "{poll}");
        service.shutdown().await;
    }
    // The direct refresh reports which accounts gained a valid reading.
    stamp(setup::ONE, NOW);
    stamp(setup::TWO, NOW - 601.0);
    let service = compose(TestClock::new(false));
    assert_eq!(
        service
            .account_service
            .refresh_accounts(600.0)
            .await
            .unwrap(),
        [setup::TWO.to_owned()]
    );
    let lease = service.accounts.lease_named(setup::TWO).unwrap().unwrap();
    stamp(setup::TWO, NOW - 601.0);
    assert!(
        service
            .account_service
            .refresh_accounts(600.0)
            .await
            .unwrap()
            .is_empty()
    );
    drop(lease);
    // An unreadable panel is not a valid update.
    std::fs::write(
        std::env::var_os("FAKE_AGY_SCRIPT").unwrap(),
        json!({"metadata_by_slot":{setup::TWO:{"quota_error":"fixture panel failure"}}})
            .to_string(),
    )
    .unwrap();
    stamp(setup::TWO, NOW - 601.0);
    assert!(
        service
            .account_service
            .refresh_accounts(600.0)
            .await
            .unwrap()
            .is_empty()
    );
    setup::evidence(
        "poll-freshness",
        json!({"invalid":600,"oneMinute":120,"fiveMinutes":300,"onlyOlderThanMaxAge":true,"busySkipped":true}),
    );
    service.shutdown().await;
}

/// The prompts of the agy turns the fake agent recorded, in order.
fn turn_prompts() -> Vec<String> {
    calls()
        .iter()
        .filter_map(|row| setup::argument(row, "-p").map(str::to_owned))
        .collect()
}

#[tokio::test]
async fn loops_and_daily_schedules_run_agy_turns_when_the_fake_clock_reaches_them() {
    let name = "loops_and_daily_schedules_run_agy_turns_when_the_fake_clock_reaches_them";
    if !worker(name, json!({})) {
        return;
    }
    let clock = TestClock::new(false);
    let service = compose(clock.clone());
    let set = |at: f64| *clock.now.lock().unwrap() = at;
    let id = service
        .create(
            std::env::current_dir().unwrap().to_str().unwrap(),
            "Scheduled work",
            "gemini-3.8-flash-high",
            Mode::Default,
        )
        .unwrap()
        .id;
    service
        .set_loop(&id, 600, "Check the nightly build.", None)
        .unwrap();
    service.scheduler_tick();
    idle(&service).await;
    assert_eq!(turn_prompts().len(), 1);
    assert!(turn_prompts()[0].ends_with("Check the nightly build."));
    set(NOW + 599.0);
    service.scheduler_tick();
    idle(&service).await;
    assert_eq!(turn_prompts().len(), 1, "a second early runs nothing");
    set(NOW + 600.0);
    service.scheduler_tick();
    idle(&service).await;
    assert_eq!(turn_prompts().len(), 2);
    let at = service
        .set_loop(&id, 0, "Write the morning report.", Some("03:00"))
        .unwrap()
        .unwrap();
    assert!(at > NOW + 600.0 && at <= NOW + 600.0 + 86_400.0);
    set(at - 1.0);
    service.scheduler_tick();
    idle(&service).await;
    assert_eq!(turn_prompts().len(), 2);
    set(at);
    service.scheduler_tick();
    idle(&service).await;
    let prompts = turn_prompts();
    assert_eq!(prompts.len(), 3);
    assert!(prompts[2].ends_with("Write the morning report."));
    let (meta, events) = service.get(&id).unwrap();
    assert_eq!(meta.loop_config.unwrap()["next"], json!(at + 86_400.0));
    let kinds: Vec<&str> = events
        .iter()
        .filter_map(|r| match &r.event {
            Event::User { kind, .. } => Some(kind.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(kinds, ["loop", "loop", "loop"]);
    service.shutdown().await;
}

#[tokio::test]
async fn an_engine_cooldown_keeps_the_account_out_until_it_expires() {
    let name = "an_engine_cooldown_keeps_the_account_out_until_it_expires";
    if !worker(name, json!({"scenarios_by_slot":{setup::ONE:"transient"}})) {
        return;
    }
    let clock = TestClock::new(false);
    let service = compose(clock.clone());
    let slots = || -> Vec<String> {
        calls()
            .iter()
            .map(|row| row["slot"].as_str().unwrap().to_owned())
            .collect()
    };
    // Two retries fail on the first account, which then rests 300 seconds from NOW + 40.
    send(&service, "First request.");
    idle(&service).await;
    assert_eq!(slots(), [setup::ONE, setup::ONE, setup::ONE, setup::TWO]);
    assert_eq!(
        service.account_overview().unwrap()[0].walls["gemini"],
        NOW + 340.0
    );
    // While it rests, a new session goes straight to the second account.
    *clock.now.lock().unwrap() = NOW + 339.0;
    send(&service, "Second request.");
    idle(&service).await;
    assert_eq!(slots()[4..], [setup::TWO]);
    // Once the rest ends, the first account is picked again.
    *clock.now.lock().unwrap() = NOW + 340.0;
    send(&service, "Third request.");
    idle(&service).await;
    assert_eq!(slots()[5], setup::ONE);
    let picks: Vec<(f64, String)> = routing_rows()
        .iter()
        .filter(|row| row["event"] == "pick")
        .map(|row| {
            (
                row["ts"].as_f64().unwrap(),
                row["slot"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert_eq!(
        picks[..4],
        [
            (NOW, setup::ONE.to_owned()),
            (NOW + 40.0, setup::TWO.to_owned()),
            (NOW + 339.0, setup::TWO.to_owned()),
            (NOW + 340.0, setup::ONE.to_owned()),
        ]
    );
    service.shutdown().await;
}
