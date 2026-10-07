//! Owns deterministic zero-cost metadata and synthetic sign-in script settings.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Metadata {
    pub quota: Option<Value>,
    pub quota_after_turn: Option<Value>,
    pub quota_error: Option<String>,
    pub quota_delay_ms: u64,
    pub inherited_pipe_ms: Option<u64>,
    pub descendant_marker: Option<String>,
    pub verify_url: Option<String>,
    pub identity: Option<String>,
    pub models: Vec<Model>,
    pub skills: Vec<String>,
    pub hooks: Vec<String>,
    pub config: Value,
}
impl Default for Metadata {
    fn default() -> Self {
        Self {
            quota: None,
            quota_after_turn: None,
            quota_error: None,
            quota_delay_ms: 0,
            inherited_pipe_ms: None,
            descendant_marker: None,
            verify_url: None,
            identity: None,
            models: vec![Model {
                id: "gemini-3.8-flash-high".into(),
                label: "Gemini 3.8 Flash High".into(),
            }],
            skills: Vec::new(),
            hooks: vec!["antiquota".into()],
            config: json!({}),
        }
    }
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Model {
    pub id: String,
    pub label: String,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Login {
    pub email: String,
    pub code: String,
    pub url: String,
    pub error: Option<String>,
    pub verify_url: Option<String>,
    pub auto_complete_ms: Option<u64>,
}
impl Default for Login {
    fn default() -> Self {
        Self {
            email: "new@example.com".into(),
            code: "good-code".into(),
            url: "https://accounts.google.com/o/oauth2/auth?client_id=stub&state=stub".into(),
            error: None,
            verify_url: None,
            auto_complete_ms: None,
        }
    }
}

/// Mirrors the real quota command envelope, including independent model baskets.
pub fn healthy_quota(gemini_fraction: f64) -> Value {
    let groups: Vec<_> = ["Gemini", "Claude/GPT"]
        .iter()
        .map(|name| {
            json!({"name": name, "buckets": [
                {"window": "5h", "remaining_fraction": if *name == "Gemini" {gemini_fraction} else {0.9}, "reset_time": "2099-01-01T05:00:00Z"},
                {"window": "weekly", "remaining_fraction": 0.9, "reset_time": "2099-01-07T00:00:00Z"}
            ]})
        })
        .collect();
    json!({"status": "SUCCESS", "command": {"name": "quota", "data": {"groups": groups}}})
}
