//! Owns small adapter contracts using only domain and standard-library types.
use crate::{
    Account, Attachment, Event, EventRecord, LoginLink, LoginState, Model, ProbeResult, Quota,
    RoutingEntry, Session, Settings, Slot, StepUsage, TurnRequest,
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
/// One read-only, fresh-conversation agy call, such as the goal verifier.
#[derive(Debug, Clone)]
pub struct AskRequest {
    pub prompt: String,
    pub model: String,
    pub cwd: String,
    pub slot: Slot,
}
/// `status` is SUCCESS, quota, auth or error; `response` is agy's reply text.
#[derive(Debug, Clone, Default)]
pub struct AskResult {
    pub status: String,
    pub response: String,
    pub error: String,
}
/// A shell command's result; `code` is `None` when it timed out or was cancelled.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CommandOutcome {
    pub code: Option<i32>,
    pub output: String,
    pub timed_out: bool,
}
/// Runs a user-configured shell command (such as a goal check) in a project folder.
pub trait ShellRunner: Send + Sync {
    fn run<'a>(
        &'a self,
        cwd: &'a str,
        command: &'a str,
        timeout: f64,
        cancel: Cancellation,
    ) -> PortFuture<'a, CommandOutcome>;
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
    /// A separate read-only call in a fresh conversation, without antiquota's session hooks.
    fn ask<'a>(&'a self, _request: AskRequest, _cancel: Cancellation) -> PortFuture<'a, AskResult> {
        Box::pin(async { unavailable() })
    }
}
pub trait AccountLease: Send + Sync {
    fn slot(&self) -> &Slot;
}
pub trait AccountStore: Send + Sync {
    fn accounts(&self) -> Result<Vec<Account>, PortError>;
    fn acquire(&self, preferred: Option<&str>) -> Result<Option<Box<dyn AccountLease>>, PortError>;
    fn accounts_at(&self, _now: f64) -> Result<Vec<Account>, PortError> {
        self.accounts()
    }
    fn lease_named(&self, name: &str) -> Result<Option<Box<dyn AccountLease>>, PortError> {
        self.acquire(Some(name))
    }
    fn prepare(&self, _slot: &Slot) -> Result<(), PortError> {
        Ok(())
    }
    fn slot(&self, _name: &str) -> Result<Slot, PortError> {
        Err(PortError::Invalid("account is unavailable".into()))
    }
    fn pause(&self, _name: &str, _on: bool) -> Result<(), PortError> {
        unavailable()
    }
    fn remove(&self, _name: &str) -> Result<(), PortError> {
        unavailable()
    }
    fn save_quota(&self, _name: &str, _quota: &Quota, _now: f64) -> Result<(), PortError> {
        unavailable()
    }
    fn set_wall(
        &self,
        _name: &str,
        _bucket: &str,
        _until: f64,
        _why: &str,
    ) -> Result<(), PortError> {
        unavailable()
    }
    fn clear_wall(&self, _name: &str, _bucket: &str) -> Result<(), PortError> {
        unavailable()
    }
    fn set_verify(&self, _name: &str, _url: Option<&str>, _now: f64) -> Result<(), PortError> {
        unavailable()
    }
    fn routes(&self, _n: usize) -> Result<Vec<RoutingEntry>, PortError> {
        Ok(vec![])
    }
    fn record_route(&self, _entry: &RoutingEntry) -> Result<(), PortError> {
        unavailable()
    }
    fn cached_models(&self) -> Result<Vec<Model>, PortError> {
        Ok(vec![])
    }
    fn save_models(&self, _models: &[Model]) -> Result<(), PortError> {
        unavailable()
    }
    fn login_start(&self) -> PortFuture<'_, LoginLink> {
        Box::pin(async { unavailable() })
    }
    fn login_state(&self, _id: &str) -> Result<LoginState, PortError> {
        unavailable()
    }
    fn login_submit<'a>(&'a self, _id: &'a str, _code: &'a str) -> PortFuture<'a, LoginState> {
        Box::pin(async { unavailable() })
    }
    fn login_cancel<'a>(&'a self, _id: &'a str) -> PortFuture<'a, ()> {
        Box::pin(async { unavailable() })
    }
    fn cancel_logins(&self) -> PortFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
    fn stack_overview(&self) -> Result<serde_json::Value, PortError> {
        unavailable()
    }
    /// antiquota's own allow and deny lists, shared by every account and session.
    fn permissions(&self) -> Result<crate::Permissions, PortError> {
        Ok(crate::Permissions::default())
    }
    /// Replaces the given lists and returns the stored permission object.
    fn save_permissions(
        &self,
        _allow: Option<Vec<String>>,
        _deny: Option<Vec<String>>,
    ) -> Result<serde_json::Value, PortError> {
        unavailable()
    }
    /// Adds one permanent allow rule; callers have already refused unsafe rules.
    fn add_allow_rule(&self, _rule: &str) -> Result<(), PortError> {
        unavailable()
    }
}
pub trait QuotaProbe: Send + Sync {
    fn probe<'a>(&'a self, slot: &'a Slot) -> PortFuture<'a, ProbeResult>;
    fn models<'a>(&'a self, _slot: &'a Slot) -> PortFuture<'a, Vec<Model>> {
        Box::pin(async { unavailable() })
    }
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
    /// Absolute paths of the mentioned files or folders that exist, relative to `cwd`.
    fn resolve_mentions(&self, _cwd: &str, _candidates: &[String]) -> Vec<String> {
        vec![]
    }
    fn settings(&self) -> Result<Option<Settings>, PortError> {
        Ok(None)
    }
    fn save_settings(&self, _settings: &Settings) -> Result<(), PortError> {
        unavailable()
    }
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
    /// The local wall-clock time of an instant as HH:MM, for messages to the user.
    fn local_hhmm(&self, _instant: f64) -> String {
        "?".into()
    }
    fn wait(&self, _seconds: f64, _cancel: Cancellation) -> PortFuture<'_, ()> {
        Box::pin(async { unavailable() })
    }
}
pub trait Notifier: Send + Sync {
    fn notify(&self, title: &str, body: &str) -> Result<(), PortError>;
}

fn unavailable<T>() -> Result<T, PortError> {
    Err(PortError::Unavailable(
        "adapter operation is unavailable".into(),
    ))
}
