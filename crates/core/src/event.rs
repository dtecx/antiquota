//! Owns the single event enum and compatible JSON/SSE envelopes.
use crate::{Account, Approval, Attachment, Session, StepUsage};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use ts_rs::TS;
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Event {
    Meta {
        meta: Box<Session>,
    },
    User {
        text: String,
        kind: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        attachments: Option<Vec<Attachment>>,
        /// Delivered to agy mid-turn through the PreInvocation hook.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        steered: Option<bool>,
    },
    TurnStart {
        turn: u64,
        slot: String,
        model: String,
    },
    Delta {
        turn: u64,
        step: u64,
        text: String,
    },
    Assistant {
        turn: u64,
        step: u64,
        text: String,
    },
    Thinking {
        turn: u64,
        step: u64,
        text: String,
        #[serde(default)]
        tokens: u64,
    },
    Tool {
        turn: u64,
        step: u64,
        name: String,
        #[serde(default)]
        params: Value,
        state: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        secs: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        output: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        diff: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        error: Option<Value>,
    },
    TurnEnd {
        turn: u64,
        slot: String,
        status: String,
        usage: StepUsage,
        secs: f64,
    },
    Notice {
        kind: String,
        text: String,
        #[serde(default)]
        data: Value,
    },
    Accounts {
        accounts: Vec<Account>,
    },
    Approval {
        #[serde(flatten)]
        approval: Approval,
    },
    Job {
        phase: String,
        job: Value,
    },
    Shell {
        cmd: String,
        output: String,
        code: Option<i32>,
        secs: f64,
    },
    Deleted {},
}
impl Event {
    pub fn is_transcript(&self) -> bool {
        if matches!(self, Self::Tool {state,..} if state=="running") {
            return false;
        }
        !matches!(
            self,
            Self::Meta { .. } | Self::Delta { .. } | Self::Accounts { .. } | Self::Deleted {}
        )
    }
    pub fn is_durable(&self) -> bool {
        if matches!(self, Self::Tool {state,..} if state=="running") {
            return false;
        }
        !matches!(self, Self::Delta { .. } | Self::Accounts { .. })
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct EventRecord {
    pub ts: f64,
    #[serde(flatten)]
    pub event: Event,
    #[serde(flatten)]
    #[ts(skip)]
    pub extra: BTreeMap<String, Value>,
}
impl EventRecord {
    pub fn new(ts: f64, event: Event) -> Self {
        Self {
            ts,
            event,
            extra: BTreeMap::new(),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct StreamEvent {
    pub sid: Option<String>,
    #[serde(flatten)]
    pub record: EventRecord,
}
