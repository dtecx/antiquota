//! Owns HTTP request/response types generated for the UI.
use antiquota_core::{Account, EventRecord, Mode, Session, Settings};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ApiError {
    pub error: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct HealthResponse {
    pub ok: bool,
    pub app: String,
    pub version: String,
    pub pid: u32,
}
pub use antiquota_core::Model;
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct EngineSummary {
    pub version: String,
    pub agy: String,
    pub busy: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct StateResponse {
    pub sessions: Vec<Session>,
    pub accounts: Vec<Account>,
    pub models: Vec<Model>,
    pub modes: Vec<Mode>,
    pub defaults: RuntimeDefaults,
    pub engine: EngineSummary,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SessionResponse {
    pub meta: Session,
    pub events: Vec<EventRecord>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(default)]
pub struct CreateSessionRequest {
    #[ts(optional)]
    pub cwd: Option<String>,
    #[ts(optional)]
    pub title: Option<String>,
    #[ts(optional)]
    pub model: Option<String>,
    #[ts(optional)]
    pub mode: Option<Mode>,
    #[ts(optional)]
    pub worktree: Option<bool>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(default)]
pub struct PatchSessionRequest {
    #[ts(optional)]
    pub effort: Option<String>,
    #[ts(optional)]
    pub title: Option<String>,
    #[ts(optional)]
    pub model: Option<String>,
    #[ts(optional)]
    pub mode: Option<Mode>,
    #[ts(optional)]
    pub pinned: Option<bool>,
    #[ts(optional)]
    pub archived: Option<bool>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(default)]
pub struct SendRequest {
    pub text: String,
    #[ts(optional)]
    pub attachments: Option<Vec<String>>,
    #[ts(optional)]
    pub kind: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SendResponse {
    pub result: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct OkResponse {
    pub ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct RuntimeDefaults {
    #[serde(flatten)]
    pub settings: Settings,
    pub cwd: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(default)]
pub struct PauseAccountRequest {
    #[ts(optional)]
    pub on: Option<bool>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct LoginCodeRequest {
    pub code: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct StackItem {
    pub name: String,
    pub here: bool,
    pub about: String,
    pub cli: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub ide: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub enabled: Option<bool>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct StackComparison {
    pub cli: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct PermissionOverview {
    pub allow: Vec<String>,
    pub deny: Vec<String>,
    pub cli: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SettingsOverview {
    pub path: String,
    pub skills: Vec<StackItem>,
    pub plugins: Vec<StackItem>,
    pub rules: Vec<StackItem>,
    pub hooks: Vec<StackItem>,
    pub mcps: Vec<StackItem>,
    pub gemini_md: StackComparison,
    pub permissions: PermissionOverview,
    pub defaults: Settings,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(default)]
pub struct DefaultsRequest {
    #[ts(optional)]
    pub model: Option<serde_json::Value>,
    #[ts(optional)]
    pub mode: Option<serde_json::Value>,
    #[ts(optional)]
    pub verifier_model: Option<serde_json::Value>,
    #[ts(optional)]
    pub rotation: Option<serde_json::Value>,
    #[ts(optional)]
    pub reserve: Option<serde_json::Value>,
    #[ts(optional)]
    pub quota_poll: Option<serde_json::Value>,
    #[ts(optional)]
    pub keep_awake: Option<serde_json::Value>,
    #[ts(optional)]
    pub approval_timeout: Option<serde_json::Value>,
    #[ts(optional)]
    pub max_rounds: Option<serde_json::Value>,
    #[ts(optional)]
    pub auto_continue: Option<serde_json::Value>,
    #[ts(optional)]
    pub placement: Option<serde_json::Value>,
    #[ts(optional)]
    pub default_cwd: Option<serde_json::Value>,
    #[ts(optional)]
    pub worktree_default: Option<serde_json::Value>,
}

/// A permission decision for the hook, or the id of a request that waits for the user.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct ApprovalResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub decision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub pending: Option<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct ApprovalRequest {
    pub session: String,
    pub tool: String,
    pub args: serde_json::Value,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct ApprovalAnswerRequest {
    pub decision: String,
    #[ts(optional)]
    pub always: Option<bool>,
    #[ts(optional)]
    pub why: Option<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct SteerResponse {
    pub messages: Vec<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct AllowRulesResponse {
    pub allow: Vec<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct PermissionsRequest {
    #[ts(optional)]
    pub add_allow: Option<Vec<String>>,
    #[ts(optional)]
    pub allow: Option<Vec<String>>,
    #[ts(optional)]
    pub deny: Option<Vec<String>>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct GoalRequest {
    pub objective: String,
    #[ts(optional)]
    pub check: Option<String>,
    #[ts(optional)]
    pub verifier: Option<bool>,
    #[ts(optional)]
    pub verifier_model: Option<String>,
    #[ts(optional)]
    pub max_rounds: Option<u64>,
}
/// Typed browser inputs; the transport keeps legacy loose-value compatibility.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct JobStartRequest {
    pub cmd: String,
    #[ts(optional)]
    pub every: Option<String>,
    #[ts(optional)]
    pub notify: Option<bool>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct LoopRequest {
    pub prompt: String,
    #[ts(optional)]
    pub every: Option<String>,
    #[ts(optional)]
    pub at: Option<String>,
}
/// A job with the end of its log, for the log view.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct JobLogResponse {
    #[serde(flatten)]
    pub job: antiquota_core::jobs::Job,
    pub text: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct LoopResponse {
    pub ok: bool,
    #[ts(optional)]
    pub next: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct TextResponse {
    pub text: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct StackFileResponse {
    pub path: String,
    pub text: String,
    pub exists: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct NoteResponse {
    pub path: String,
    pub file: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct FilesResponse {
    pub files: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct FolderResponse {
    pub path: String,
    pub parent: Option<String>,
    pub dirs: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SearchHit {
    pub id: String,
    pub title: String,
    pub cwd: String,
    pub ts: f64,
    pub who: String,
    pub snippet: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct UsageDay {
    pub date: String,
    pub total: u64,
    pub turns: u64,
    pub by: std::collections::BTreeMap<String, u64>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct UsageAccount {
    pub tokens: u64,
    pub turns: u64,
    pub today: u64,
    pub input: u64,
    pub output: u64,
}
/// Token counts split into their parts; `cache` is the input agy reused from its cache.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct UsageParts {
    pub input: u64,
    pub output: u64,
    pub thinking: u64,
    pub cache: u64,
    pub turns: u64,
}
/// The period, today, each day (in `days` order), each account and each model, by part.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct UsageDetail {
    pub period: UsageParts,
    pub today: UsageParts,
    pub days: Vec<UsageParts>,
    pub accounts: std::collections::BTreeMap<String, UsageParts>,
    pub models: std::collections::BTreeMap<String, UsageParts>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct UsageResponse {
    pub days: Vec<UsageDay>,
    pub accounts: std::collections::BTreeMap<String, UsageAccount>,
    pub models: std::collections::BTreeMap<String, u64>,
    /// Present only when the request asks for `detail=1`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub detail: Option<UsageDetail>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct StackResponse {
    pub skills: Option<Vec<String>>,
    pub hooks: Option<Vec<String>>,
    pub mcp: Vec<String>,
    pub path: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct EngineResponse {
    pub version: String,
    pub pid: u32,
    pub url: Option<String>,
    pub by: Option<String>,
    pub started: Option<f64>,
    pub base: String,
    pub stack: String,
    pub log: String,
    pub hook: bool,
    pub agy: String,
    pub sessions: usize,
    pub accounts: usize,
}
