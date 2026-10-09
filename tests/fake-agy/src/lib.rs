//! Scripted agy protocol scenarios, independent of production adapters.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub mod metadata;
pub mod protocol;

#[derive(Debug, Default, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Scenario {
    #[default]
    Success,
    Delay,
    Quota,
    Rate,
    Auth,
    Approval,
    Hook,
    Background,
    GoalComplete,
    Huge,
    Crash,
    StaleResult,
    DuplicateDone,
    Transient,
    ConversationLost,
    QuotaNoInit,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Action {
    pub event: Value,
    #[serde(default)]
    pub delay_ms: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Script {
    pub scenario: Scenario,
    pub scenarios_by_slot: std::collections::BTreeMap<String, Scenario>,
    pub scenarios_by_turn: Vec<Scenario>,
    pub scenarios_by_slot_and_turn: std::collections::BTreeMap<String, Vec<Scenario>>,
    pub metadata: metadata::Metadata,
    pub metadata_by_slot: std::collections::BTreeMap<String, metadata::Metadata>,
    pub login: metadata::Login,
    pub delay_ms: u64,
    pub huge_bytes: usize,
    pub reply: Option<String>,
    pub background_command: String,
    pub verdict_done: bool,
    pub done_after: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// Cached prompt and thinking tokens of the reply step; left out of the frame while zero.
    pub cache_read_tokens: u64,
    pub thinking_tokens: u64,
    pub result_input_tokens: u64,
    pub result_output_tokens: u64,
    pub hook_command: Vec<String>,
    pub hook_payload: Value,
    pub actions: Vec<Action>,
    pub exit_code: Option<i32>,
    pub write_notes: bool,
    pub output_inherited_pipe_ms: Option<u64>,
    pub output_descendant_marker: Option<String>,
    /// Bytes of stderr written, after two non-JSON stdout lines, before the first frame.
    pub noise_bytes: usize,
    /// Text written to stderr after the last frame, such as warnings on a successful turn.
    pub stderr: Option<String>,
}

impl Default for Script {
    fn default() -> Self {
        Self {
            scenario: Scenario::Success,
            scenarios_by_slot: Default::default(),
            scenarios_by_turn: Vec::new(),
            scenarios_by_slot_and_turn: Default::default(),
            metadata: Default::default(),
            metadata_by_slot: Default::default(),
            login: Default::default(),
            delay_ms: 0,
            huge_bytes: 2_000_000,
            reply: None,
            background_command: "printf 'background fixture\\n'".into(),
            verdict_done: true,
            done_after: 1,
            input_tokens: 100,
            output_tokens: 10,
            cache_read_tokens: 0,
            thinking_tokens: 0,
            result_input_tokens: 9_999,
            result_output_tokens: 999,
            hook_command: Vec::new(),
            hook_payload: json!({"toolCall":{"name":"run_command","parameters":{"CommandLine":"touch fake-hook-marker"}}}),
            actions: Vec::new(),
            exit_code: None,
            write_notes: true,
            output_inherited_pipe_ms: None,
            output_descendant_marker: None,
            noise_bytes: 0,
            stderr: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TurnContext {
    pub slot: String,
    pub cwd: String,
    pub conversation: String,
    pub turn: u64,
}

impl Script {
    pub fn selected(&self, ctx: &TurnContext) -> Scenario {
        self.scenarios_by_slot_and_turn
            .get(&ctx.slot)
            .and_then(|turns| turns.get(ctx.turn.saturating_sub(1) as usize))
            .copied()
            .or_else(|| self.scenarios_by_slot.get(&ctx.slot).copied())
            .or_else(|| {
                self.scenarios_by_turn
                    .get(ctx.turn.saturating_sub(1) as usize)
                    .copied()
            })
            .unwrap_or(self.scenario)
    }
    pub fn metadata_for(&self, slot: &str) -> &metadata::Metadata {
        self.metadata_by_slot.get(slot).unwrap_or(&self.metadata)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FakeError {
    #[error("fake agy requires an isolated Desktop/test data root: {0}")]
    Isolation(String),
    #[error("fixture I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("fixture JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid scenario: {0}")]
    Scenario(String),
}
