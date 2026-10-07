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
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Model {
    pub id: String,
    pub label: String,
}
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
