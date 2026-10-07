//! Owns account, slot, and quota data without credential access.
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
pub struct Account {
    pub slot: String,
    pub busy: bool,
    pub probed: f64,
    pub quota: Quota,
    pub walls: BTreeMap<String, f64>,
    pub why: BTreeMap<String, String>,
    pub paused: bool,
    pub verify: Option<Value>,
    pub today: Value,
}
