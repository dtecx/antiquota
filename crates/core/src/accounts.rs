//! Owns public account and quota values without paths to private credentials.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use ts_rs::TS;
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct Quota(pub BTreeMap<String, BTreeMap<String, (f64, Option<f64>)>>);
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct Slot {
    pub name: String,
    pub home: String,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct TodayUsage {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub input: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub turns: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub output: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub thinking: Option<u64>,
    /// Input agy reused from its cache; present only when some was reused today.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cache: Option<u64>,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct Account {
    pub slot: String,
    pub busy: bool,
    pub probed: Option<f64>,
    pub quota: Quota,
    pub walls: BTreeMap<String, f64>,
    pub why: BTreeMap<String, String>,
    pub paused: bool,
    pub verify: Option<String>,
    pub today: TodayUsage,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct RoutingEntry {
    pub ts: f64,
    pub event: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub slot: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub bucket: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub session: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub previous: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub why: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub until: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub secs: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub wait: Option<f64>,
    #[serde(flatten)]
    #[ts(skip)]
    pub extra: BTreeMap<String, Value>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct Model {
    pub id: String,
    pub label: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct LoginLink {
    pub id: String,
    pub url: String,
    pub seconds: u32,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum SignInState {
    #[default]
    Waiting,
    Done,
    Failed,
    Verify,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct LoginState {
    pub state: SignInState,
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub name: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub verify_url: Option<Option<String>>,
}
impl LoginState {
    pub fn waiting() -> Self {
        Self {
            name: Some(None),
            verify_url: Some(None),
            ..Self::default()
        }
    }
    pub fn replaced() -> Self {
        Self {
            state: SignInState::Failed,
            error: Some("replaced by a newer sign-in link (only the newest one works)".into()),
            ..Self::default()
        }
    }
}
#[derive(Debug, Clone, PartialEq)]
pub enum ProbeResult {
    Quota(Quota),
    Verify(String),
    SignedOut,
    Unreadable,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    Quota,
    Auth,
    Transient,
    /// The machine cannot reach Google at all; no account is at fault.
    Offline,
}

/// Validates an existing slot name before a transport or adapter resolves its path.
pub fn validate_account_name(name: &str) -> Result<(), crate::PortError> {
    if name.is_empty()
        || name.len() > 100
        || name.starts_with('.')
        || !name
            .bytes()
            .all(|x| x.is_ascii_alphanumeric() || b"@._%+-".contains(&x))
    {
        return Err(crate::PortError::Invalid(format!(
            "not an account: '{name}'"
        )));
    }
    Ok(())
}
