//! Proves dictation's account choice, waiting, prompt and cleanup with in-memory ports.
use super::*;
use antiquota_core::{Account, AskRequest, AskResult, Slot};

#[derive(Default)]
struct Recordings {
    inner: MemoryStore,
    saved: Mutex<Vec<usize>>,
    discarded: Mutex<Vec<String>>,
}
impl SessionStore for Recordings {
    fn load(&self) -> Result<Vec<(Session, Vec<EventRecord>)>, PortError> {
        self.inner.load()
    }
    fn append(&self, id: &str, record: &EventRecord) -> Result<(), PortError> {
        self.inner.append(id, record)
    }
    fn save(&self, session: &Session) -> Result<(), PortError> {
        self.inner.save(session)
    }
    fn delete(&self, id: &str) -> Result<(), PortError> {
        self.inner.delete(id)
    }
    fn exists(&self, id: &str) -> Result<bool, PortError> {
        self.inner.exists(id)
    }
    fn validate_cwd(&self, cwd: &str) -> Result<String, PortError> {
        self.inner.validate_cwd(cwd)
    }
    fn attachments(&self, id: &str, paths: &[String]) -> Result<Vec<Attachment>, PortError> {
        self.inner.attachments(id, paths)
    }
    fn turn_timestamp(
        &self,
        id: &str,
        now: f64,
        records: &[EventRecord],
    ) -> Result<f64, PortError> {
        self.inner.turn_timestamp(id, now, records)
    }
    fn project_usage(&self, id: &str, records: &[EventRecord]) -> Result<(), PortError> {
        self.inner.project_usage(id, records)
    }
    fn state_dir(&self) -> String {
        self.inner.state_dir()
    }
    fn save_recording(&self, data: &[u8], extension: &str) -> Result<(String, String), PortError> {
        self.saved.lock().unwrap().push(data.len());
        Ok(("/rec".into(), format!("/rec/clip.{extension}")))
    }
    fn discard_recording(&self, path: &str) {
        self.discarded.lock().unwrap().push(path.into());
    }
}

/// One Gemini account that is busy for the first `busy` lease attempts.
struct Busy {
    busy: Mutex<u32>,
    walled: bool,
}
impl AccountStore for Busy {
    fn accounts(&self) -> Result<Vec<Account>, PortError> {
        self.accounts_at(0.0)
    }
    fn accounts_at(&self, _now: f64) -> Result<Vec<Account>, PortError> {
        Ok(vec![Account {
            slot: "fake".into(),
            walls: if self.walled {
                [("gemini".to_owned(), f64::MAX)].into()
            } else {
                Default::default()
            },
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
        let mut busy = self.busy.lock().unwrap();
        if *busy > 0 {
            *busy -= 1;
            return Ok(None);
        }
        Ok(Some(Box::new(Lease(Slot {
            name: "fake".into(),
            home: "memory".into(),
        }))))
    }
}

struct Asker {
    answer: AskResult,
    asked: Mutex<Vec<AskRequest>>,
}
impl AgentRunner for Asker {
    fn run<'a>(
        &'a self,
        _request: TurnRequest,
        _cancel: Cancellation,
        _sink: Box<dyn antiquota_core::TurnSink + 'a>,
    ) -> PortFuture<'a, TurnResult> {
        Box::pin(async { Ok(TurnResult::default()) })
    }
    fn ask<'a>(&'a self, request: AskRequest, _cancel: Cancellation) -> PortFuture<'a, AskResult> {
        self.asked.lock().unwrap().push(request);
        let answer = self.answer.clone();
        Box::pin(async move { Ok(answer) })
    }
}

/// Advances time instantly on every wait and records each wait.
#[derive(Default)]
struct Skipping {
    now: Mutex<f64>,
    waits: Mutex<Vec<f64>>,
}
impl Clock for Skipping {
    fn now(&self) -> f64 {
        *self.now.lock().unwrap()
    }
    fn wait(&self, seconds: f64, _cancel: Cancellation) -> PortFuture<'_, ()> {
        self.waits.lock().unwrap().push(seconds);
        *self.now.lock().unwrap() += seconds;
        Box::pin(async { Ok(()) })
    }
}

struct Rig {
    service: Arc<Service>,
    store: Arc<Recordings>,
    runner: Arc<Asker>,
    clock: Arc<Skipping>,
}
fn rig(busy: u32, walled: bool, status: &str, response: &str) -> Rig {
    let store = Arc::new(Recordings::default());
    let runner = Arc::new(Asker {
        answer: AskResult {
            status: status.into(),
            response: response.into(),
            error: String::new(),
        },
        asked: Mutex::new(vec![]),
    });
    let clock = Arc::new(Skipping::default());
    let service = Service::open(
        store.clone(),
        Settings::default(),
        Arc::new(Busy {
            busy: Mutex::new(busy),
            walled,
        }),
        runner.clone(),
        clock.clone(),
    )
    .unwrap();
    Rig {
        service,
        store,
        runner,
        clock,
    }
}

#[tokio::test]
async fn a_busy_account_is_awaited_then_the_recording_is_transcribed_and_deleted() {
    let rig = rig(2, false, "SUCCESS", "  Hello there.  \n");
    let heard = rig.service.transcribe(b"RIFF", "webm").await.unwrap();
    assert_eq!(
        heard,
        crate::Transcript {
            text: "Hello there.".into(),
            slot: "fake".into()
        }
    );
    assert_eq!(*rig.clock.waits.lock().unwrap(), vec![1.0, 1.0]);
    assert_eq!(*rig.store.discarded.lock().unwrap(), vec!["/rec/clip.webm"]);
    let asked = rig.runner.asked.lock().unwrap();
    let request = &asked[0];
    assert!(
        request.prompt.starts_with(
            "Listen to the audio file /rec/clip.webm with your file viewer; do not run commands or open other files. If it contains clear human speech"
        )
    );
    assert!(request.prompt.ends_with("Never guess or invent words."));
    assert_eq!(request.model, antiquota_core::DEFAULT_MODEL);
    assert_eq!(request.cwd, "/rec");
    assert_eq!(request.add_dirs, vec!["/rec".to_owned()]);
    assert_eq!(request.timeout, 180.0);
    assert_eq!(request.slot.name, "fake");
}

#[tokio::test]
async fn silence_failures_and_limits_answer_without_leaving_recordings() {
    let quiet = rig(0, false, "SUCCESS", "NO_SPEECH");
    assert_eq!(
        quiet.service.transcribe(b"x", "wav").await.unwrap().text,
        ""
    );
    let failed = rig(0, false, "error", "");
    assert_eq!(
        failed
            .service
            .transcribe(b"x", "wav")
            .await
            .unwrap_err()
            .to_string(),
        "agy could not transcribe the recording"
    );
    assert_eq!(failed.store.discarded.lock().unwrap().len(), 1);
    let empty = rig(0, false, "SUCCESS", "text");
    for data in [Vec::new(), vec![0; crate::RECORDING_LIMIT + 1]] {
        assert_eq!(
            empty
                .service
                .transcribe(&data, "wav")
                .await
                .unwrap_err()
                .to_string(),
            "record between a second and a few minutes"
        );
    }
    assert!(empty.store.saved.lock().unwrap().is_empty());
    let walled = rig(0, true, "SUCCESS", "text");
    assert_eq!(
        walled
            .service
            .transcribe(b"x", "wav")
            .await
            .unwrap_err()
            .to_string(),
        "no account has Gemini quota for dictation right now"
    );
    assert_eq!(walled.store.discarded.lock().unwrap().len(), 1);
    let stuck = rig(u32::MAX, false, "SUCCESS", "text");
    assert_eq!(
        stuck
            .service
            .transcribe(b"x", "wav")
            .await
            .unwrap_err()
            .to_string(),
        "every Gemini account is busy right now; try again in a moment"
    );
    assert_eq!(stuck.clock.waits.lock().unwrap().len(), 120);
    assert!(stuck.runner.asked.lock().unwrap().is_empty());
}
