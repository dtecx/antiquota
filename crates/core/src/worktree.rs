//! Owns a session's git worktree record and branch naming.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use ts_rs::TS;

/// Where a worktree session works and what it was branched from; unknown fields are kept.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct Worktree {
    pub repo: String,
    pub path: String,
    pub branch: String,
    pub base: String,
    pub base_sha: String,
    #[serde(flatten)]
    #[ts(skip)]
    pub extra: BTreeMap<String, Value>,
}
impl Worktree {
    pub fn from_value(value: &Value) -> Option<Self> {
        serde_json::from_value(value.clone()).ok()
    }
    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}

/// Lowercase words joined by `-`, at most 32 characters, or "session".
pub fn slug(text: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in text.to_lowercase().chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            if dash && !out.is_empty() {
                out.push('-');
            }
            dash = false;
            out.push(c);
        } else {
            dash = true;
        }
    }
    let cut: String = out.chars().take(32).collect();
    let cut = cut.trim_matches('-').to_owned();
    if cut.is_empty() {
        "session".into()
    } else {
        cut
    }
}

/// The branch of a session's worktree: `antiquota/<title-slug>-<first four id characters>`.
pub fn branch_name(title: &str, sid: &str) -> String {
    format!("antiquota/{}-{}", slug(title), &sid[..sid.len().min(4)])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn branches_use_a_short_title_slug_and_the_session_prefix() {
        assert_eq!(slug("Fix the Login bug!"), "fix-the-login-bug");
        assert_eq!(slug("  ---  "), "session");
        assert_eq!(slug("Ünïcode only"), "n-code-only");
        assert_eq!(slug(&"word ".repeat(20)).len(), 32);
        assert!(!slug(&"ab ".repeat(20)).ends_with('-'));
        assert_eq!(branch_name("", "abcdef123456"), "antiquota/session-abcd");
        assert_eq!(branch_name("Add tests", "1234"), "antiquota/add-tests-1234");
    }
}
