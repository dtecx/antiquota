//! Proves keep-awake decisions and local daily schedule times with in-memory ports.
use super::*;
use std::sync::atomic::AtomicBool;

#[derive(Default)]
struct RecordingAwake {
    held: AtomicBool,
    changes: AtomicU64,
}
impl antiquota_core::Awake for RecordingAwake {
    fn hold(&self) {
        if !self.held.swap(true, Ordering::SeqCst) {
            self.changes.fetch_add(1, Ordering::SeqCst);
        }
    }
    fn release(&self) {
        if self.held.swap(false, Ordering::SeqCst) {
            self.changes.fetch_add(1, Ordering::SeqCst);
        }
    }
}

fn awake_service(keep_awake: &str, awake: Arc<RecordingAwake>) -> Arc<Service> {
    Service::compose(crate::Ports {
        store: Arc::new(MemoryStore::default()),
        settings: Settings {
            keep_awake: keep_awake.into(),
            ..Default::default()
        },
        accounts: Arc::new(Accounts),
        runner: Arc::new(Runner {
            calls: AtomicU64::new(0),
        }),
        clock: Arc::new(FixedClock),
        quota: None,
        shell: None,
        jobs: None,
        awake: Some(awake),
    })
    .unwrap()
}

#[tokio::test]
async fn the_computer_stays_awake_only_while_work_runs() {
    let awake = Arc::new(RecordingAwake::default());
    let service = awake_service("on", awake.clone());
    let id = service
        .create("memory", "", "model", antiquota_core::Mode::Default)
        .unwrap()
        .id;
    service.scheduler_tick();
    assert!(!awake.held.load(Ordering::SeqCst));
    service
        .send(&id, "work".into(), vec![], "user".into())
        .unwrap();
    service.scheduler_tick();
    assert!(awake.held.load(Ordering::SeqCst), "held while a turn runs");
    service.wait_idle(&id).await;
    service.scheduler_tick();
    assert!(!awake.held.load(Ordering::SeqCst), "released once idle");
    assert_eq!(awake.changes.load(Ordering::SeqCst), 2);

    let off = Arc::new(RecordingAwake::default());
    let service = awake_service("off", off.clone());
    let id = service
        .create("memory", "", "model", antiquota_core::Mode::Default)
        .unwrap()
        .id;
    service
        .send(&id, "work".into(), vec![], "user".into())
        .unwrap();
    service.scheduler_tick();
    assert!(
        !off.held.load(Ordering::SeqCst),
        "keep_awake=off never holds"
    );
    service.wait_idle(&id).await;
}

#[test]
fn daily_schedules_use_the_local_calendar() {
    use chrono::{Local, TimeZone};
    let clock = SystemClock;
    let noon = Local
        .with_ymd_and_hms(2026, 10, 7, 12, 0, 0)
        .earliest()
        .unwrap()
        .timestamp() as f64;
    let later = Local
        .with_ymd_and_hms(2026, 10, 7, 13, 30, 0)
        .earliest()
        .unwrap()
        .timestamp() as f64;
    let tomorrow = Local
        .with_ymd_and_hms(2026, 10, 8, 12, 0, 0)
        .earliest()
        .unwrap()
        .timestamp() as f64;
    assert_eq!(clock.next_daily(13, 30, noon), later);
    assert_eq!(
        clock.next_daily(12, 0, noon),
        tomorrow,
        "equal time means tomorrow"
    );
    assert_eq!(clock.local_hhmm(later), "13:30");
}

#[tokio::test]
async fn loop_validation_matches_the_legacy_messages() {
    let service = awake_service("on", Arc::new(RecordingAwake::default()));
    let id = service
        .create("memory", "", "model", antiquota_core::Mode::Default)
        .unwrap()
        .id;
    let prompt_error = service
        .set_loop(&id, 600, "  ", None)
        .unwrap_err()
        .to_string();
    assert_eq!(
        prompt_error,
        "what should it send? /loop 10m <prompt> or /schedule 03:00 <prompt>"
    );
    assert_eq!(
        service
            .set_loop(&id, 0, "x", Some("24:00"))
            .unwrap_err()
            .to_string(),
        "time like 03:00 or 18:30"
    );
    assert_eq!(service.set_loop(&id, 600, "tick", None).unwrap(), None);
    let config = service.get(&id).unwrap().0.loop_config.unwrap();
    assert_eq!(config["every"], 600);
    assert_eq!(
        config["next"], 10.0,
        "an interval loop runs first right away"
    );
    service.set_loop(&id, 0, "", None).unwrap();
    assert_eq!(service.get(&id).unwrap().0.loop_config, None);
}

/// Jobs that never end; pids count up.
#[derive(Default)]
struct EndlessJobs {
    next: AtomicU64,
}
impl antiquota_core::JobRunner for EndlessJobs {
    fn start(&self, _cwd: &str, _command: &str, _log: &str) -> Result<u32, PortError> {
        Ok(100 + self.next.fetch_add(1, Ordering::SeqCst) as u32)
    }
    fn wait<'a>(&'a self, _pid: u32) -> PortFuture<'a, Option<i32>> {
        Box::pin(std::future::pending())
    }
    fn stop(&self, _pid: u32) -> Result<(), PortError> {
        Ok(())
    }
    fn read_log(&self, _log: &str, _max_bytes: usize) -> Result<String, PortError> {
        Ok(String::new())
    }
}

#[tokio::test]
async fn sessions_keep_twenty_jobs_and_skip_duplicate_requests() {
    let awake = Arc::new(RecordingAwake::default());
    let service = Service::compose(crate::Ports {
        store: Arc::new(MemoryStore::default()),
        settings: Settings::default(),
        accounts: Arc::new(Accounts),
        runner: Arc::new(Runner {
            calls: AtomicU64::new(0),
        }),
        clock: Arc::new(FixedClock),
        quota: None,
        shell: None,
        jobs: Some(Arc::new(EndlessJobs::default())),
        awake: Some(awake.clone()),
    })
    .unwrap();
    let id = service
        .create("memory", "", "model", antiquota_core::Mode::Bypass)
        .unwrap()
        .id;
    for n in 0..21 {
        let job = service
            .job_start(&id, &format!("job {n}"), 0, false, "user")
            .unwrap();
        assert_eq!(job.id.len(), 8);
    }
    let jobs = service.get(&id).unwrap().0.jobs;
    assert_eq!(jobs.len(), 20);
    assert_eq!(jobs[0]["cmd"], "job 1", "the oldest record goes first");
    assert_eq!(service.running_jobs(), 21 - 1);
    service
        .jobs_from_reply(
            &id,
            "RUN_IN_BACKGROUND: job 5\nRUN_IN_BACKGROUND: make bench",
        )
        .unwrap();
    let cmds: Vec<String> = service
        .get(&id)
        .unwrap()
        .0
        .jobs
        .iter()
        .map(|j| j["cmd"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(
        cmds.iter().filter(|c| *c == "job 5").count(),
        1,
        "a running duplicate is skipped"
    );
    assert_eq!(cmds.last().unwrap(), "make bench");
    assert!(service.busy_count() >= 20);
    service.scheduler_tick();
    assert!(
        awake.held.load(Ordering::SeqCst),
        "running jobs keep the computer awake"
    );
    service.shutdown().await;
    assert!(!awake.held.load(Ordering::SeqCst), "shutdown releases it");
}
