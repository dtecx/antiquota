//! Proves deletion admission and registry liveness during blocked store operations.
use super::*;
use std::sync::{Barrier, atomic::AtomicBool};
struct BlockingStore {
    inner: MemoryStore,
    block_save: AtomicBool,
    entered: Barrier,
    release: Barrier,
}
impl BlockingStore {
    fn new() -> Self {
        Self {
            inner: MemoryStore::default(),
            block_save: AtomicBool::new(false),
            entered: Barrier::new(2),
            release: Barrier::new(2),
        }
    }
}
impl SessionStore for BlockingStore {
    fn load(&self) -> Result<Vec<(Session, Vec<EventRecord>)>, PortError> {
        self.inner.load()
    }
    fn append(&self, id: &str, record: &EventRecord) -> Result<(), PortError> {
        self.inner.append(id, record)
    }
    fn save(&self, session: &Session) -> Result<(), PortError> {
        if self.block_save.swap(false, Ordering::SeqCst) {
            self.entered.wait();
            self.release.wait();
        }
        self.inner.save(session)
    }
    fn delete(&self, id: &str) -> Result<(), PortError> {
        self.entered.wait();
        self.release.wait();
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
}
fn service(store: Arc<BlockingStore>) -> Arc<Service> {
    Service::open(
        store,
        Settings::default(),
        Arc::new(Accounts),
        Arc::new(Runner {
            calls: AtomicU64::new(0),
        }),
        Arc::new(FixedClock),
    )
    .unwrap()
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn deletion_refuses_admission_through_a_previously_cloned_cell() {
    let store = Arc::new(BlockingStore::new());
    let service = service(store.clone());
    let session = service
        .create("memory", "deletion", "model", antiquota_core::Mode::Default)
        .unwrap();
    let stale = service.cell(&session.id).unwrap();
    let deleting = service.clone();
    let id = session.id.clone();
    let deletion = tokio::spawn(async move { deleting.delete(&id).await });
    store.entered.wait();
    assert!(service.cells.read().unwrap().contains_key(&session.id));
    let started = Arc::new(Barrier::new(2));
    let sending = service.clone();
    let id = session.id.clone();
    let ready = started.clone();
    let handle = tokio::runtime::Handle::current();
    let attempt = std::thread::spawn(move || {
        let _runtime = handle.enter();
        ready.wait();
        sending.send_cell(
            &id,
            stale,
            "Must not resurrect".into(),
            vec![],
            "user".into(),
        )
    });
    started.wait();
    store.release.wait();
    deletion.await.unwrap().unwrap();
    assert!(matches!(attempt.join().unwrap(),Err(AppError::Missing(id)) if id==session.id));
    assert!(store.inner.load().unwrap().is_empty());
    assert!(service.get(&session.id).is_err());
    assert_eq!(service.busy_count(), 0);
    let restarted = Service::open(
        store,
        Settings::default(),
        Arc::new(Accounts),
        Arc::new(Runner {
            calls: AtomicU64::new(0),
        }),
        Arc::new(FixedClock),
    )
    .unwrap();
    assert!(restarted.sessions().unwrap().is_empty());
}
#[test]
fn creation_store_io_does_not_block_existing_session_lookup() {
    let store = Arc::new(BlockingStore::new());
    let service = service(store.clone());
    let existing = service
        .create("memory", "existing", "model", antiquota_core::Mode::Default)
        .unwrap();
    store.block_save.store(true, Ordering::SeqCst);
    let creating = service.clone();
    let creation = std::thread::spawn(move || {
        creating.create("memory", "new", "model", antiquota_core::Mode::Default)
    });
    store.entered.wait();
    let reading = service.clone();
    let id = existing.id.clone();
    let (sender, receiver) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        sender.send(reading.get(&id)).unwrap();
    });
    let result = receiver.recv_timeout(std::time::Duration::from_secs(1));
    store.release.wait();
    creation.join().unwrap().unwrap();
    reader.join().unwrap();
    assert_eq!(result.unwrap().unwrap().0.id, existing.id);
    assert_eq!(service.sessions().unwrap().len(), 2);
}
