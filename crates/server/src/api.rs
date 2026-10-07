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
    pub allow: Option<Vec<String>>,
    #[ts(optional)]
    pub deny: Option<Vec<String>>,
}
