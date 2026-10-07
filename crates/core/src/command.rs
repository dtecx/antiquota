//! Owns typed application commands, patch fields, and turn requests.
use crate::{Mode, Slot};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(default)]
pub struct SessionPatch {
    #[ts(optional)]
    pub title: Option<String>,
    #[ts(optional)]
    pub model: Option<String>,
    #[ts(optional)]
    pub effort: Option<String>,
    #[ts(optional)]
    pub mode: Option<Mode>,
    #[ts(optional)]
    pub pinned: Option<bool>,
    #[ts(optional)]
    pub archived: Option<bool>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum Command {
    Create {
        cwd: String,
        title: String,
        model: String,
        mode: Mode,
    },
    Patch {
        session: String,
        fields: SessionPatch,
    },
    Send {
        session: String,
        text: String,
        attachments: Vec<String>,
        kind: String,
    },
    Stop {
        session: String,
    },
    Delete {
        session: String,
    },
    SetAwake {
        on: bool,
    },
}
#[derive(Debug, Clone)]
pub struct TurnRequest {
    pub session: String,
    pub turn: u64,
    pub prompt: String,
    pub model: String,
    pub cwd: String,
    pub mode: Mode,
    pub conversation_id: Option<String>,
    pub slot: Slot,
    pub state_dir: String,
}
