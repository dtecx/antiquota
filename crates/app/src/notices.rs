//! Adds core-vetted command suggestions to current and historical denial notices.
use antiquota_core::policy::suggest_rule;
use serde_json::{Value, json};

pub(crate) fn denial_suggestions(data: &mut Value) {
    let mut rules = Vec::new();
    if let Some(commands) = data.get("commands").and_then(Value::as_array) {
        for command in commands.iter().filter_map(Value::as_str) {
            let rule = suggest_rule(command);
            if !rule.is_empty() && !rules.contains(&rule) {
                rules.push(rule);
            }
        }
    }
    if !data.is_object() {
        *data = json!({});
    }
    data["rules"] = json!(rules);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn suggestions_replace_untrusted_history_and_keep_other_notice_fields() {
        let mut data = json!({"commands":["git status", "git status", "/bin/bash -c echo", "python3 -c pass", "find . -exec sh {} ;"], "actions":[], "rules":["command(*)"], "future":123});
        denial_suggestions(&mut data);
        assert_eq!(data["rules"], json!(["command(git status)"]));
        assert_eq!(data["future"], 123);
        assert_eq!(data["actions"], json!([]));
        for mut malformed in [
            Value::Null,
            json!(false),
            json!({"commands":[false,null,3]}),
        ] {
            denial_suggestions(&mut malformed);
            assert_eq!(malformed["rules"], json!([]));
        }
    }
}
