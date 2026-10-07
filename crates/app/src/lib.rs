//! Owns application services and session serialization; adapters own all I/O.
mod admission;
mod commands;
pub use commands::CommandResult;
mod history;
mod runner_sink;
mod sessions;
pub use sessions::mode as parse_mode;
mod turns;
use antiquota_core::{
    AccountStore, AgentRunner, Cancellation, Clock, EventRecord, PortError, Session, SessionStore,
    Settings, StreamEvent,
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
    pub(crate) runner: Arc<dyn AgentRunner>,
    pub(crate) clock: Arc<dyn Clock>,
    pub(crate) reserved: Mutex<BTreeSet<String>>,
    pub(crate) cells: RwLock<BTreeMap<String, Arc<Cell>>>,
    pub(crate) events: broadcast::Sender<StreamEvent>,
}
pub struct SystemClock;
impl Clock for SystemClock {
    fn now(&self) -> f64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64()
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
        Ok(Arc::new(Self {
            store,
            accounts,
            settings,
            runner,
            clock,
            reserved: Mutex::new(BTreeSet::new()),
            cells: RwLock::new(cells),
            events,
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
    pub async fn shutdown(&self) {
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
