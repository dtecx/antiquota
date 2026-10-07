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
/// Detached background jobs: they log to a file, survive the engine, and stop as a tree.
pub trait JobRunner: Send + Sync {
    /// Starts `command` with the platform shell in `cwd`, appending its output to `log`;
    /// returns its process id.
    fn start(&self, cwd: &str, command: &str, log: &str) -> Result<u32, PortError>;
    /// Resolves when the job ends: its exit code for jobs this engine started, `None` for jobs
    /// adopted after a restart (their status cannot be read).
    fn wait<'a>(&'a self, pid: u32) -> PortFuture<'a, Option<i32>>;
    /// Ends the job's whole process tree.
    fn stop(&self, pid: u32) -> Result<(), PortError>;
    /// The last `max_bytes` of a log file, empty when it does not exist yet.
    fn read_log(&self, log: &str, max_bytes: usize) -> Result<String, PortError>;
}
/// Keeps the computer awake (not the display) while agents or jobs run.
pub trait Awake: Send + Sync {
    fn hold(&self);
    fn release(&self);
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
    /// Where this session's uploaded attachments live.
    fn attachments_dir(&self, _sid: &str) -> String {
        String::new()
    }
    /// The folder that holds session worktrees.
    fn worktrees_dir(&self) -> String {
        String::new()
    }
    /// Where a job of this session writes its log.
    fn job_log_path(&self, _sid: &str, _jid: &str) -> String {
        String::new()
    }
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
/// A session's project folder as the Files tab and file viewer see it.
pub trait Workspace: Send + Sync {
    /// One folder of the project: folders first, hidden and heavy entries last and dimmed,
    /// at most 400 entries; `{root, path, entries}`.
    fn tree(&self, cwd: &str, path: &str) -> Result<serde_json::Value, PortError>;
    /// A text file inside the project or one of `roots`, at most 2 MB; `{path, rel, text}`.
    fn read_text(
        &self,
        cwd: &str,
        roots: &[String],
        path: &str,
    ) -> Result<serde_json::Value, PortError>;
}
/// Git for sessions: worktrees, review, commit and merge. Nothing here pushes anywhere.
pub trait Vcs: Send + Sync {
    /// The repository's top folder, or `None` outside a repository.
    fn root(&self, cwd: &str) -> Option<String>;
    /// A new branch and worktree starting at the current commit; returns the record and the
    /// folder the session works in (the same subfolder as `cwd`).
    fn add_worktree(
        &self,
        cwd: &str,
        branch: &str,
        folder: &str,
    ) -> Result<(crate::worktree::Worktree, String), PortError>;
    /// Branch, changed files with counts, and for a worktree what it adds to its base;
    /// `None` outside a repository.
    fn status(
        &self,
        cwd: &str,
        worktree: Option<&crate::worktree::Worktree>,
    ) -> Result<Option<serde_json::Value>, PortError>;
    fn diff(
        &self,
        cwd: &str,
        path: Option<&str>,
        committed: Option<&crate::worktree::Worktree>,
    ) -> Result<String, PortError>;
    /// Commits every change and returns the short commit id.
    fn commit(&self, cwd: &str, message: &str) -> Result<String, PortError>;
    fn discard(&self, cwd: &str, path: &str) -> Result<(), PortError>;
    /// Merges the worktree branch into its base in the main checkout; returns the short id.
    fn merge(
        &self,
        worktree: &crate::worktree::Worktree,
        message: &str,
    ) -> Result<String, PortError>;
    fn remove_worktree(
        &self,
        worktree: &crate::worktree::Worktree,
        delete_branch: bool,
    ) -> Result<(), PortError>;
    /// Renames a worktree's branch; `true` when the checkout now has the new name.
    fn rename_branch(&self, path: &str, from: &str, to: &str) -> bool;
}
/// What the Terminal panel shows about a session's shell.
#[derive(Debug, Clone, PartialEq, serde::Serialize, ts_rs::TS)]
pub struct TerminalInfo {
    pub alive: bool,
    pub shell: String,
    pub cwd: String,
}
/// Output for one subscriber: raw bytes, then `Exit` once the shell ends.
#[derive(Debug, Clone, PartialEq)]
pub enum TerminalChunk {
    Data(Vec<u8>),
    Exit,
}
/// The recent screen for a new subscriber and the chunks that follow it.
pub struct TerminalFeed {
    pub recent: Vec<u8>,
    pub chunks: std::sync::mpsc::Receiver<TerminalChunk>,
}
/// One interactive shell in a pseudo-terminal.
pub trait Terminal: Send + Sync {
    fn info(&self) -> TerminalInfo;
    /// Sends keyboard input; fails once the shell has exited.
    fn write(&self, data: &[u8]) -> Result<(), PortError>;
    /// Resizes a live terminal; resizing an ended one does nothing.
    fn resize(&self, cols: u16, rows: u16) -> Result<(), PortError>;
    /// Ends the shell and everything it started; ending twice is harmless.
    fn kill(&self);
    fn subscribe(&self) -> TerminalFeed;
}
/// Starts shells for the Terminal panel.
pub trait Terminals: Send + Sync {
    fn spawn(&self, cwd: &str, cols: u16, rows: u16) -> Result<Arc<dyn Terminal>, PortError>;
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
    /// The next instant a daily local time is reached: later today, or tomorrow when it is now
    /// or has passed. The default uses UTC days.
    fn next_daily(&self, hour: u32, minute: u32, now: f64) -> f64 {
        let day = (now / 86400.0).floor() * 86400.0;
        let at = day + f64::from(hour * 3600 + minute * 60);
        if at <= now { at + 86400.0 } else { at }
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
