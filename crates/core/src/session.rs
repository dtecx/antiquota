//! Owns compatible session snapshots and turn metadata.
use crate::{DEFAULT_MODEL, Goal};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use ts_rs::TS;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    #[default]
    Default,
    AcceptEdits,
    Plan,
    Bypass,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
pub struct Usage {
    pub input: u64,
    pub output: u64,
    pub thinking: u64,
    #[serde(flatten)]
    #[ts(skip)]
    pub extra: BTreeMap<String, Value>,
}
fn is_zero(n: &u64) -> bool {
    *n == 0
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
pub struct StepUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub thinking_tokens: u64,
    pub total_tokens: u64,
    pub context: u64,
    /// Prompt tokens agy read from its cache, on top of `input_tokens`. Left out while zero.
    #[serde(skip_serializing_if = "is_zero")]
    #[ts(as = "Option<u64>", optional)]
    pub cache_read_tokens: u64,
    #[serde(flatten)]
    #[ts(skip)]
    pub extra: BTreeMap<String, Value>,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct Attachment {
    pub name: String,
    pub path: String,
    pub image: bool,
    /// Bytes, reported on upload; transcript records leave it out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub size: Option<u64>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub cwd: String,
    pub model: String,
    pub effort: String,
    pub mode: Mode,
    pub created: f64,
    pub updated: f64,
    pub status: String,
    pub status_until: Option<f64>,
    pub conversation_id: Option<String>,
    pub turns: u64,
    pub queue: Vec<Value>,
    pub goal: Option<Goal>,
    pub last_slot: Option<String>,
    pub usage: Usage,
    pub context: u64,
    pub pending_note: String,
    pub jobs: Vec<Value>,
    #[serde(rename = "loop")]
    pub loop_config: Option<Value>,
    pub pinned: bool,
    pub archived: bool,
    pub worktree: Option<Value>,
    #[serde(flatten)]
    #[ts(skip)]
    pub extra: BTreeMap<String, Value>,
}
impl Default for Session {
    fn default() -> Self {
        Self {
            id: String::new(),
            title: String::new(),
            cwd: String::new(),
            model: DEFAULT_MODEL.into(),
            effort: String::new(),
            mode: Mode::Default,
            created: 0.0,
            updated: 0.0,
            status: "idle".into(),
            status_until: None,
            conversation_id: None,
            turns: 0,
            queue: vec![],
            goal: None,
            last_slot: None,
            usage: Usage::default(),
            context: 0,
            pending_note: String::new(),
            jobs: vec![],
            loop_config: None,
            pinned: false,
            archived: false,
            worktree: None,
            extra: BTreeMap::new(),
        }
    }
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct Turn {
    pub session: String,
    pub number: u64,
    pub slot: String,
    pub model: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub retries: u32,
    #[serde(default)]
    pub failovers: u32,
}
