//! Owns compatible settings, rules, goals, and approval data.
use crate::DEFAULT_MODEL;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use ts_rs::TS;
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
pub struct Goal {
    pub objective: String,
    pub check: String,
    pub verifier: bool,
    pub verifier_model: String,
    pub max_rounds: u64,
    pub round: u64,
    pub state: String,
    #[serde(flatten)]
    #[ts(skip)]
    pub extra: BTreeMap<String, Value>,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct Approval {
    pub id: String,
    pub session: String,
    pub tool: String,
    pub args: Value,
    pub rule: String,
    pub state: String,
    pub why: String,
    pub deadline: f64,
    #[serde(flatten)]
    #[ts(skip)]
    pub extra: BTreeMap<String, Value>,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct Rule {
    pub text: String,
    pub deny: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
pub struct Settings {
    pub model: String,
    pub mode: String,
    pub verifier_model: String,
    pub rotation: String,
    pub keep_awake: String,
    pub reserve: String,
    pub quota_poll: String,
    pub approval_timeout: String,
    pub max_rounds: String,
    pub auto_continue: String,
    pub default_cwd: String,
    pub worktree_default: String,
    #[serde(flatten)]
    #[ts(skip)]
    pub extra: BTreeMap<String, Value>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            model: DEFAULT_MODEL.into(),
            mode: "default".into(),
            verifier_model: DEFAULT_MODEL.into(),
            rotation: "perishable".into(),
            keep_awake: "on".into(),
            reserve: "3".into(),
            quota_poll: "10".into(),
            approval_timeout: "15".into(),
            max_rounds: "20".into(),
            auto_continue: "2".into(),
            default_cwd: String::new(),
            worktree_default: "off".into(),
            extra: BTreeMap::new(),
        }
    }
}
