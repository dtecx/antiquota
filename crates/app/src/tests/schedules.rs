//! Proves that loops and daily schedules fire when a fake clock reaches them, and only then.
use super::*;

/// A clock the test moves; its days are UTC days from the default `next_daily`.
struct MovableClock(Mutex<f64>);
impl Clock for MovableClock {
    fn now(&self) -> f64 {
        *self.0.lock().unwrap()
    }
}

fn loop_prompts(service: &Service, id: &str) -> Vec<String> {
    service
        .get(id)
        .unwrap()
        .1
        .iter()
        .filter_map(|r| match &r.event {
            Event::User { text, kind, .. } if kind == "loop" => Some(text.clone()),
            _ => None,
        })
        .collect()
}
async fn idle(service: &Service, id: &str) {
    for _ in 0..200 {
        if service.get(id).unwrap().0.status == "idle" {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("the turn did not finish");
}
fn next(service: &Service, id: &str) -> f64 {
    service.get(id).unwrap().0.loop_config.unwrap()["next"]
        .as_f64()
        .unwrap()
}

#[tokio::test]
async fn loops_and_daily_schedules_fire_when_the_fake_clock_reaches_them() {
    let clock = Arc::new(MovableClock(Mutex::new(1_000.0)));
    let service = Service::open(
        Arc::new(MemoryStore::default()),
        Settings::default(),
        Arc::new(Accounts),
        Arc::new(Runner {
            calls: AtomicU64::new(0),
        }),
        clock.clone(),
    )
    .unwrap();
    let set = |at: f64| *clock.0.lock().unwrap() = at;
    let id = service
        .create("memory", "", "model", antiquota_core::Mode::Default)
        .unwrap()
        .id;
    // A loop is due at once, then every interval measured by the clock.
    assert_eq!(
        service.set_loop(&id, 900, "check the build", None).unwrap(),
        None
    );
    service.scheduler_tick();
    idle(&service, &id).await;
    assert_eq!(loop_prompts(&service, &id), ["check the build"]);
    assert_eq!(next(&service, &id), 1_900.0);
    set(1_899.0);
    service.scheduler_tick();
    assert_eq!(loop_prompts(&service, &id).len(), 1, "one second early");
    set(1_900.0);
    service.scheduler_tick();
    idle(&service, &id).await;
    assert_eq!(loop_prompts(&service, &id).len(), 2);
    assert_eq!(next(&service, &id), 2_800.0);
    // An interval below a minute is raised to a minute.
    service.set_loop(&id, 5, "too often", None).unwrap();
    service.scheduler_tick();
    idle(&service, &id).await;
    assert_eq!(next(&service, &id), 1_960.0);
    // A busy session lets a due loop wait; it fires on the first tick after the turn.
    set(1_960.0);
    service
        .send(&id, "user work".into(), vec![], "user".into())
        .unwrap();
    service.scheduler_tick();
    assert_eq!(
        loop_prompts(&service, &id).len(),
        3,
        "not while a turn runs"
    );
    idle(&service, &id).await;
    service.scheduler_tick();
    idle(&service, &id).await;
    assert_eq!(loop_prompts(&service, &id).len(), 4);
    // A daily schedule waits for its time of day, then moves on by one day.
    let at = service
        .set_loop(&id, 0, "nightly report", Some("03:00"))
        .unwrap()
        .unwrap();
    assert_eq!(at, 3.0 * 3_600.0);
    set(at - 1.0);
    service.scheduler_tick();
    assert_eq!(loop_prompts(&service, &id).len(), 4);
    set(at);
    service.scheduler_tick();
    idle(&service, &id).await;
    assert_eq!(
        loop_prompts(&service, &id).last().unwrap(),
        "nightly report"
    );
    assert_eq!(next(&service, &id), at + 86_400.0);
    // A time already past today is scheduled for tomorrow.
    set(4.0 * 3_600.0);
    let tomorrow = service
        .set_loop(&id, 0, "nightly report", Some("03:00"))
        .unwrap()
        .unwrap();
    assert_eq!(tomorrow, 86_400.0 + 3.0 * 3_600.0);
    // Clearing the loop stops it for good.
    service.set_loop(&id, 0, "", None).unwrap();
    set(1_000_000.0);
    service.scheduler_tick();
    assert_eq!(loop_prompts(&service, &id).len(), 5);
    assert!(service.get(&id).unwrap().0.loop_config.is_none());
}
