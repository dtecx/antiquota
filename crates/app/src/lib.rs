//! Owns application services and session serialization; adapters own all I/O.
mod admission;
mod approvals;
pub use approvals::ApprovalReply;
mod commands;
pub use commands::CommandResult;
mod history;
mod runner_sink;
mod sessions;
pub use sessions::mode as parse_mode;
mod account_service;
mod accounts;
mod attempt;
mod turns;
pub use account_service::AccountService;
mod defaults;
mod goals;
pub use goals::TurnOutcome;
mod handoff;
use antiquota_core::{
    AccountStore, AgentRunner, Cancellation, Clock, CommandOutcome, EventRecord, PortError,
    PortFuture, QuotaProbe, Session, SessionStore, Settings, ShellRunner, StreamEvent,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex, RwLock},
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::broadcast;
#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("not found: '{0}'")]
    Missing(String),
    #[error("{0}")]
    Invalid(String),
    #[error(transparent)]
    Port(#[from] PortError),
}
pub(crate) struct SessionState {
    meta: Session,
    records: Vec<EventRecord>,
    busy: bool,
    cancel: Cancellation,
    deleting: bool,
}
pub(crate) struct Cell {
    state: Mutex<SessionState>,
}
pub struct Service {
    pub store: Arc<dyn SessionStore>,
    pub accounts: Arc<dyn AccountStore>,
    pub settings: Settings,
    pub account_service: Arc<AccountService>,
    pub(crate) runner: Arc<dyn AgentRunner>,
    pub(crate) clock: Arc<dyn Clock>,
    pub(crate) shell: Arc<dyn ShellRunner>,
    pub(crate) reserved: Mutex<BTreeSet<String>>,
    pub(crate) cells: RwLock<BTreeMap<String, Arc<Cell>>>,
    pub(crate) events: broadcast::Sender<StreamEvent>,
    pub(crate) approvals: Mutex<BTreeMap<String, antiquota_core::Approval>>,
    pub(crate) approval_signal: tokio::sync::Notify,
}
pub struct SystemClock;
impl Clock for SystemClock {
    fn wait(&self, seconds: f64, cancel: Cancellation) -> antiquota_core::PortFuture<'_, ()> {
        Box::pin(async move {
            let deadline =
                tokio::time::Instant::now() + std::time::Duration::from_secs_f64(seconds.max(0.0));
            while !cancel.is_cancelled() && tokio::time::Instant::now() < deadline {
                tokio::time::sleep(
                    std::time::Duration::from_millis(20)
                        .min(deadline.saturating_duration_since(tokio::time::Instant::now())),
                )
                .await;
            }
            Ok(())
        })
    }
    fn now(&self) -> f64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64()
    }
    fn local_hhmm(&self, instant: f64) -> String {
        if instant <= 0.0 || !instant.is_finite() {
            return "?".into();
        }
        chrono::DateTime::from_timestamp(instant as i64, 0)
            .map(|t| t.with_timezone(&chrono::Local).format("%H:%M").to_string())
            .unwrap_or_else(|| "?".into())
    }
}

/// Used until a shell adapter is supplied: goal checks report that they cannot run.
struct NoShell;
impl ShellRunner for NoShell {
    fn run<'a>(
        &'a self,
        _cwd: &'a str,
        _command: &'a str,
        _timeout: f64,
        _cancel: Cancellation,
    ) -> PortFuture<'a, CommandOutcome> {
        Box::pin(async {
            Err(PortError::Unavailable(
                "no shell is available to run the check".into(),
            ))
        })
    }
}

/// Every adapter the service needs; `shell` and `quota` are optional for tests.
pub struct Ports {
    pub store: Arc<dyn SessionStore>,
    pub settings: Settings,
    pub accounts: Arc<dyn AccountStore>,
    pub runner: Arc<dyn AgentRunner>,
    pub clock: Arc<dyn Clock>,
    pub quota: Option<Arc<dyn QuotaProbe>>,
    pub shell: Option<Arc<dyn ShellRunner>>,
}

/// Python-compatible truthiness for loosely typed stored values.
pub(crate) fn http_truthy(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => false,
        serde_json::Value::Bool(b) => *b,
        serde_json::Value::Number(n) => n.as_f64() != Some(0.0),
        serde_json::Value::String(s) => !s.is_empty(),
        serde_json::Value::Array(a) => !a.is_empty(),
        serde_json::Value::Object(o) => !o.is_empty(),
    }
}
impl Service {
    pub fn open(
        store: Arc<dyn SessionStore>,
        settings: Settings,
        accounts: Arc<dyn AccountStore>,
        runner: Arc<dyn AgentRunner>,
        clock: Arc<dyn Clock>,
    ) -> Result<Arc<Self>, AppError> {
        Self::open_with_quota(store, settings, accounts, runner, clock, None)
    }
    pub fn open_with_quota(
        store: Arc<dyn SessionStore>,
        settings: Settings,
        accounts: Arc<dyn AccountStore>,
        runner: Arc<dyn AgentRunner>,
        clock: Arc<dyn Clock>,
        quota: Option<Arc<dyn antiquota_core::QuotaProbe>>,
    ) -> Result<Arc<Self>, AppError> {
        Self::compose(Ports {
            store,
            settings,
            accounts,
            runner,
            clock,
            quota,
            shell: None,
        })
    }
    pub fn compose(ports: Ports) -> Result<Arc<Self>, AppError> {
        let Ports {
            store,
            settings,
            accounts,
            runner,
            clock,
            quota,
            shell,
        } = ports;
        let mut cells = BTreeMap::new();
        for (mut meta, records) in store.load()? {
            store.project_usage(&meta.id, &records)?;
            meta.status = "idle".into();
            meta.status_until = None;
            cells.insert(
                meta.id.clone(),
                Arc::new(Cell {
                    state: Mutex::new(SessionState {
                        meta,
                        records,
                        busy: false,
                        cancel: Cancellation::default(),
                        deleting: false,
                    }),
                }),
            );
        }
        let (events, _) = broadcast::channel(1024);
        let account_service = AccountService::with_events(
            store.clone(),
            settings.clone(),
            accounts.clone(),
            clock.clone(),
            quota,
            events.clone(),
        );
        Ok(Arc::new(Self {
            store,
            accounts,
            settings,
            account_service,
            runner,
            clock,
            shell: shell.unwrap_or_else(|| Arc::new(NoShell)),
            reserved: Mutex::new(BTreeSet::new()),
            cells: RwLock::new(cells),
            events,
            approvals: Mutex::new(BTreeMap::new()),
            approval_signal: tokio::sync::Notify::new(),
        }))
    }
    pub fn subscribe(&self) -> broadcast::Receiver<StreamEvent> {
        self.events.subscribe()
    }
    pub(crate) fn cell(&self, id: &str) -> Result<Arc<Cell>, AppError> {
        self.cells
            .read()
            .map_err(|_| AppError::Invalid("session registry unavailable".into()))?
            .get(id)
            .cloned()
            .ok_or_else(|| AppError::Missing(id.into()))
    }
    pub fn sessions(&self) -> Result<Vec<Session>, AppError> {
        let cells: Vec<_> = self
            .cells
            .read()
            .map_err(|_| AppError::Invalid("session registry unavailable".into()))?
            .values()
            .cloned()
            .collect();
        let mut sessions = vec![];
        for cell in cells {
            let state = cell
                .state
                .lock()
                .map_err(|_| AppError::Invalid("session unavailable".into()))?;
            if !state.deleting {
                sessions.push(state.meta.clone());
            }
        }
        sessions.sort_by(|a, b| b.updated.total_cmp(&a.updated));
        Ok(sessions)
    }
    pub fn busy_count(&self) -> usize {
        let cells: Vec<_> = self
            .cells
            .read()
            .map(|cells| cells.values().cloned().collect())
            .unwrap_or_default();
        cells
            .iter()
            .filter(|c| c.state.lock().map(|s| s.busy).unwrap_or(false))
            .count()
    }
    pub fn begin_shutdown(&self) {
        self.account_service.begin_shutdown();
    }
    pub async fn shutdown(&self) {
        self.begin_shutdown();
        let _ = self.accounts.cancel_logins().await;
        let ids = self.sessions().unwrap_or_default();
        for s in &ids {
            let busy = self
                .cell(&s.id)
                .ok()
                .and_then(|cell| cell.state.lock().ok().map(|state| state.busy))
                .unwrap_or(false);
            if busy {
                let _ = self.stop(&s.id);
            }
        }
        for s in ids {
            self.wait_idle(&s.id).await;
        }
    }
    pub(crate) async fn wait_idle(&self, id: &str) {
        loop {
            let busy = self
                .cell(id)
                .ok()
                .and_then(|c| c.state.lock().ok().map(|s| s.busy))
                .unwrap_or(false);
            if !busy {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }
}

#[cfg(test)]
mod tests;

impl std::ops::Deref for Service {
    type Target = AccountService;
    fn deref(&self) -> &Self::Target {
        &self.account_service
    }
}
