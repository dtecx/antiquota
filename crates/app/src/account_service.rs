//! Owns account-only services without loading or reconciling session history.
use antiquota_core::{
    AccountStore, Cancellation, Clock, QuotaProbe, SessionStore, Settings, StreamEvent,
};
use std::sync::{Arc, Mutex, atomic::AtomicBool};
use tokio::sync::broadcast;
pub struct AccountService {
    pub store: Arc<dyn SessionStore>,
    pub accounts: Arc<dyn AccountStore>,
    pub settings: Settings,
    pub(crate) clock: Arc<dyn Clock>,
    pub(crate) quota: Option<Arc<dyn QuotaProbe>>,
    pub(crate) background_cancel: Cancellation,
    pub(crate) models_fresh: AtomicBool,
    pub(crate) settings_writer: Mutex<()>,
    pub(crate) events: broadcast::Sender<StreamEvent>,
}
impl AccountService {
    pub fn new(
        store: Arc<dyn SessionStore>,
        settings: Settings,
        accounts: Arc<dyn AccountStore>,
        clock: Arc<dyn Clock>,
        quota: Option<Arc<dyn QuotaProbe>>,
    ) -> Arc<Self> {
        let (events, _) = broadcast::channel(1024);
        Self::with_events(store, settings, accounts, clock, quota, events)
    }
    pub(crate) fn with_events(
        store: Arc<dyn SessionStore>,
        settings: Settings,
        accounts: Arc<dyn AccountStore>,
        clock: Arc<dyn Clock>,
        quota: Option<Arc<dyn QuotaProbe>>,
        events: broadcast::Sender<StreamEvent>,
    ) -> Arc<Self> {
        Arc::new(Self {
            store,
            settings,
            accounts,
            clock,
            quota,
            events,
            background_cancel: Cancellation::default(),
            models_fresh: AtomicBool::new(false),
            settings_writer: Mutex::new(()),
        })
    }
    pub fn begin_shutdown(&self) {
        self.background_cancel.cancel();
    }
    pub fn subscribe(&self) -> broadcast::Receiver<StreamEvent> {
        self.events.subscribe()
    }
}
