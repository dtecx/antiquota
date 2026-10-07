//! Proves approval deadlines, wake-ups and stop handling with an injected clock.
use super::*;
use crate::ApprovalReply;
use antiquota_core::Verdict;
use serde_json::json;
struct MovableClock(Mutex<f64>);
impl Clock for MovableClock {
    fn now(&self) -> f64 {
        *self.0.lock().unwrap()
    }
}
fn service(clock: Arc<MovableClock>, minutes: &str) -> (Arc<Service>, String) {
    let settings = Settings {
        approval_timeout: minutes.into(),
        ..Default::default()
    };
    let service = Service::open(
        Arc::new(MemoryStore::default()),
        settings,
        Arc::new(Accounts),
        Arc::new(Runner {
            calls: AtomicU64::new(0),
        }),
        clock,
    )
    .unwrap();
    let session = service
        .create("memory", "", "model", antiquota_core::Mode::Default)
        .unwrap();
    (service, session.id)
}
fn pending(service: &Service, id: &str, line: &str) -> String {
    match service
        .approval_request(id, "run_command", &json!({ "CommandLine": line }))
        .unwrap()
    {
        ApprovalReply::Pending(aid) => aid,
        other => panic!("expected a pending request, got {other:?}"),
    }
}

#[tokio::test]
async fn an_unanswered_request_is_declined_at_its_deadline() {
    let clock = Arc::new(MovableClock(Mutex::new(1_000.0)));
    let (service, id) = service(clock.clone(), "2");
    let aid = pending(&service, &id, "touch x");
    assert_eq!(service.get(&id).unwrap().0.status, "approval");
    assert_eq!(
        service.approval_wait(&aid, 0.0).await.unwrap(),
        ApprovalReply::Pending(aid.clone())
    );
    *clock.0.lock().unwrap() += 119.0;
    assert_eq!(
        service.approval_wait(&aid, 0.0).await.unwrap(),
        ApprovalReply::Pending(aid.clone())
    );
    *clock.0.lock().unwrap() += 1.0;
    let expired = ApprovalReply::Decided {
        decision: Verdict::Deny,
        reason: "No answer from the user within 2 minutes. Continue without it, or explain what you need."
            .into(),
    };
    assert_eq!(service.approval_wait(&aid, 30.0).await.unwrap(), expired);
    assert_eq!(service.approval_wait(&aid, 0.0).await.unwrap(), expired);
    let late = service.approval_answer(&aid, true, false, "").unwrap();
    assert_eq!(late, expired, "the first decision stays");
    let (meta, events) = service.get(&id).unwrap();
    assert_eq!(meta.status, "idle");
    let states: Vec<String> = events
        .iter()
        .filter_map(|r| match &r.event {
            Event::Approval { approval } => Some(approval.state.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(states, ["pending", "denied"]);
}

#[tokio::test]
async fn an_answer_wakes_a_waiting_request_at_once() {
    let clock = Arc::new(MovableClock(Mutex::new(1_000.0)));
    let (service, id) = service(clock, "15");
    let aid = pending(&service, &id, "touch x");
    let waiting = {
        let service = service.clone();
        let aid = aid.clone();
        tokio::spawn(async move {
            let started = std::time::Instant::now();
            (
                service.approval_wait(&aid, 30.0).await.unwrap(),
                started.elapsed(),
            )
        })
    };
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    service.approval_answer(&aid, false, false, "").unwrap();
    let (reply, waited) = waiting.await.unwrap();
    assert_eq!(
        reply,
        ApprovalReply::Decided {
            decision: Verdict::Deny,
            reason: antiquota_core::policy::DECLINED.into(),
        }
    );
    assert!(waited < std::time::Duration::from_secs(2));
    assert_eq!(
        service.approval_wait("unknown", 0.0).await.unwrap(),
        ApprovalReply::Decided {
            decision: Verdict::Ask,
            reason: String::new(),
        }
    );
    assert!(matches!(
        service.approval_answer("unknown", true, false, ""),
        Err(AppError::Missing(_))
    ));
}

#[tokio::test]
async fn timeouts_clamp_to_the_allowed_range_and_stop_declines() {
    let clock = Arc::new(MovableClock(Mutex::new(1_000.0)));
    let (service, id) = service(clock.clone(), "999");
    let aid = pending(&service, &id, "touch x");
    let deadline = service.approvals.lock().unwrap()[&aid].deadline;
    assert_eq!(deadline, 1_000.0 + 240.0 * 60.0);
    let second = pending(&service, &id, "touch y");
    service.stop(&id).unwrap();
    for aid in [aid, second] {
        assert_eq!(
            service.approval_wait(&aid, 0.0).await.unwrap(),
            ApprovalReply::Decided {
                decision: Verdict::Deny,
                reason: antiquota_core::policy::STOPPED.into(),
            }
        );
    }
    let (_, events) = service.get(&id).unwrap();
    assert_eq!(
        events
            .iter()
            .filter(
                |r| matches!(&r.event, Event::Approval { approval } if approval.state == "denied")
            )
            .count(),
        2
    );
}

#[tokio::test]
async fn the_session_waits_until_every_request_is_answered() {
    let clock = Arc::new(MovableClock(Mutex::new(1_000.0)));
    let (service, id) = service(clock, "15");
    let first = pending(&service, &id, "touch a");
    let second = pending(&service, &id, "touch b");
    service.approval_answer(&first, true, false, "").unwrap();
    assert_eq!(service.get(&id).unwrap().0.status, "approval");
    service.approval_answer(&second, true, false, "").unwrap();
    assert_eq!(service.get(&id).unwrap().0.status, "idle");
    let (_, events) = service.get(&id).unwrap();
    let answered: Vec<_> = events
        .iter()
        .filter_map(|r| match &r.event {
            Event::Approval { approval } if approval.state == "allowed" => {
                Some(approval.extra.get("always").cloned())
            }
            _ => None,
        })
        .collect();
    assert_eq!(answered, [Some(json!(false)), Some(json!(false))]);
}

#[tokio::test]
async fn steering_takes_plain_messages_and_leaves_attachments_queued() {
    let clock = Arc::new(MovableClock(Mutex::new(1_000.0)));
    let (service, id) = service(clock, "15");
    assert_eq!(
        service
            .send(&id, "start".into(), vec![], "user".into())
            .unwrap(),
        "started"
    );
    assert_eq!(
        service
            .send(&id, "plain".into(), vec![], "user".into())
            .unwrap(),
        "queued"
    );
    assert_eq!(
        service
            .send(&id, "with file".into(), vec!["a.txt".into()], "user".into())
            .unwrap(),
        "queued"
    );
    assert_eq!(service.steer(&id).unwrap(), ["plain"]);
    let queue = service.get(&id).unwrap().0.queue;
    assert_eq!(queue.len(), 1);
    assert_eq!(queue[0]["text"], "with file");
    assert!(service.steer(&id).unwrap().is_empty());
    service.wait_idle(&id).await;
    let (meta, events) = service.get(&id).unwrap();
    assert_eq!(meta.turns, 2, "the attachment message runs as its own turn");
    let steered = events
        .iter()
        .filter(|r| matches!(&r.event, Event::User { steered: Some(true), text, .. } if text == "plain"))
        .count();
    assert_eq!(steered, 1);
}
