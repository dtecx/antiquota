//! Owns small adapter contracts using only domain and standard-library types.
use crate::{
    Account, Attachment, Event, EventRecord, Quota, Session, Slot, StepUsage, TurnRequest,
};
use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use thiserror::Error;
#[derive(Debug, Error)]
pub enum PortError {
    #[error("{0}")]
    Unavailable(String),
    #[error("{0}")]
    Invalid(String),
}
pub type PortFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, PortError>> + Send + 'a>>;
#[derive(Debug, Clone, Default)]
pub struct Cancellation(Arc<AtomicBool>);
impl Cancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}
#[derive(Debug, Clone, Default)]
pub struct TurnResult {
    pub status: String,
    pub conversation_id: Option<String>,
    pub usage: StepUsage,
    pub error: String,
}
pub trait TurnSink: Send {
    fn event(&mut self, event: Event) -> Result<(), PortError>;
    fn conversation_id(&mut self, conversation_id: &str) -> Result<(), PortError>;
}
pub trait AgentRunner: Send + Sync {
    fn run<'a>(
        &'a self,
        request: TurnRequest,
        cancel: Cancellation,
        sink: Box<dyn TurnSink + 'a>,
    ) -> PortFuture<'a, TurnResult>;
}
pub trait AccountLease: Send {
    fn slot(&self) -> &Slot;
}
pub trait AccountStore: Send + Sync {
    fn accounts(&self) -> Result<Vec<Account>, PortError>;
    fn acquire(&self, preferred: Option<&str>) -> Result<Option<Box<dyn AccountLease>>, PortError>;
}
pub trait QuotaProbe: Send + Sync {
    fn probe<'a>(&'a self, slot: &'a Slot) -> PortFuture<'a, Quota>;
}
pub trait SessionStore: Send + Sync {
    fn load(&self) -> Result<Vec<(Session, Vec<EventRecord>)>, PortError>;
    fn append(&self, sid: &str, record: &EventRecord) -> Result<(), PortError>;
    fn save(&self, session: &Session) -> Result<(), PortError>;
    fn delete(&self, sid: &str) -> Result<(), PortError>;
    fn exists(&self, sid: &str) -> Result<bool, PortError>;
    fn validate_cwd(&self, cwd: &str) -> Result<String, PortError>;
    fn attachments(&self, sid: &str, paths: &[String]) -> Result<Vec<Attachment>, PortError>;
    fn turn_timestamp(
        &self,
        sid: &str,
        now: f64,
        records: &[EventRecord],
    ) -> Result<f64, PortError>;
    fn project_usage(&self, sid: &str, records: &[EventRecord]) -> Result<(), PortError>;
    fn state_dir(&self) -> String;
}
pub trait Vcs: Send + Sync {
    fn status(&self, cwd: &str) -> Result<String, PortError>;
}
pub trait Pty: Send {
    fn write(&mut self, data: &[u8]) -> Result<(), PortError>;
    fn resize(&mut self, cols: u16, rows: u16) -> Result<(), PortError>;
    fn stop(&mut self) -> Result<(), PortError>;
}
pub trait Transcriber: Send + Sync {
    fn transcribe<'a>(&'a self, audio: Vec<u8>, kind: &'a str) -> PortFuture<'a, String>;
}
pub trait Clock: Send + Sync {
    fn now(&self) -> f64;
}
pub trait Notifier: Send + Sync {
    fn notify(&self, title: &str, body: &str) -> Result<(), PortError>;
}
