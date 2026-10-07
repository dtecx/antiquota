//! Owns the shape of antiquota's own agy hook entry and how agy invokes it.
use serde_json::{Value, json};

/// The key of antiquota's entry in the own stack's `config/hooks.json`.
pub const HOOK_NAME: &str = "antiquota";
/// The entry key used before the rename; replaced on upgrade.
pub const OLD_HOOK_NAME: &str = "agyloop";
/// agy waits this long (seconds) for a tool decision, so the user has time to answer.
pub const PRE_TOOL_TIMEOUT: u64 = 3600;
pub const PRE_INVOCATION_TIMEOUT: u64 = 15;

fn quote(arg: &str, windows: bool) -> String {
    if windows {
        if !arg.is_empty() && !arg.contains([' ', '\t', '"']) {
            return arg.into();
        }
        let mut out = String::from("\"");
        let mut slashes = 0;
        for c in arg.chars() {
            match c {
                '\\' => slashes += 1,
                '"' => {
                    out.push_str(&"\\".repeat(slashes * 2 + 1));
                    out.push('"');
                    slashes = 0;
                }
                c => {
                    out.push_str(&"\\".repeat(slashes));
                    out.push(c);
                    slashes = 0;
                }
            }
        }
        out.push_str(&"\\".repeat(slashes * 2));
        out.push('"');
        out
    } else if !arg.is_empty()
        && arg
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "@%+=:,./-_".contains(c))
    {
        arg.into()
    } else {
        format!("'{}'", arg.replace('\'', r"'\''"))
    }
}

/// The shell command agy runs for one hook kind (`pre-tool` or `pre-invocation`).
pub fn hook_command(program: &str, kind: &str, windows: bool) -> String {
    [program, "hook", kind]
        .iter()
        .map(|arg| quote(arg, windows))
        .collect::<Vec<_>>()
        .join(" ")
}

/// antiquota's hook entry: every tool call goes through `pre-tool`, and messages sent mid-turn
/// reach agy through `pre-invocation`. `enabled` keeps the user's on/off choice.
pub fn hook_entry(program: &str, enabled: bool, windows: bool) -> Value {
    json!({
        "PreToolUse": [{"matcher": "*", "hooks": [{
            "type": "command",
            "command": hook_command(program, "pre-tool", windows),
            "timeout": PRE_TOOL_TIMEOUT,
        }]}],
        "PreInvocation": [{
            "type": "command",
            "command": hook_command(program, "pre-invocation", windows),
            "timeout": PRE_INVOCATION_TIMEOUT,
        }],
        "enabled": enabled,
    })
}

/// Rewrites antiquota's entry in a hooks.json object for this program. Returns whether it
/// changed. The old entry from before the rename is removed; a user's `enabled: false` stays.
pub fn install_hook_entry(hooks: &mut Value, program: &str, windows: bool) -> bool {
    if !hooks.is_object() {
        *hooks = json!({});
    }
    let Some(map) = hooks.as_object_mut() else {
        return false;
    };
    let stale = map.remove(OLD_HOOK_NAME);
    let old = map.get(HOOK_NAME).or(stale.as_ref()).cloned();
    let enabled = old
        .as_ref()
        .and_then(|entry| entry.get("enabled"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let entry = hook_entry(program, enabled, windows);
    if map.get(HOOK_NAME) == Some(&entry) && stale.is_none() {
        return false;
    }
    map.insert(HOOK_NAME.into(), entry);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn commands_quote_paths_for_each_platform() {
        assert_eq!(
            hook_command("/opt/antiquota/bin/antiquota", "pre-tool", false),
            "/opt/antiquota/bin/antiquota hook pre-tool"
        );
        assert_eq!(
            hook_command("/Users/a b/it's/antiquota", "pre-tool", false),
            r"'/Users/a b/it'\''s/antiquota' hook pre-tool"
        );
        assert_eq!(
            hook_command(
                r"C:\Program Files\antiquota\antiquota.exe",
                "pre-invocation",
                true
            ),
            r#""C:\Program Files\antiquota\antiquota.exe" hook pre-invocation"#
        );
        assert_eq!(
            hook_command(r"C:\x\a.exe", "pre-tool", true),
            r"C:\x\a.exe hook pre-tool"
        );
    }
    #[test]
    fn install_replaces_the_old_entry_and_keeps_a_disabled_choice() {
        let mut hooks = json!({"agyloop": {"enabled": false}, "other": {"x": 1}});
        assert!(install_hook_entry(&mut hooks, "/bin/aq", false));
        assert!(hooks.get("agyloop").is_none());
        assert_eq!(hooks["other"]["x"], 1);
        assert_eq!(hooks["antiquota"]["enabled"], false);
        assert_eq!(hooks["antiquota"]["PreToolUse"][0]["matcher"], "*");
        assert_eq!(
            hooks["antiquota"]["PreToolUse"][0]["hooks"][0]["timeout"],
            3600
        );
        assert_eq!(hooks["antiquota"]["PreInvocation"][0]["timeout"], 15);
        assert!(!install_hook_entry(&mut hooks, "/bin/aq", false));
        assert!(install_hook_entry(&mut hooks, "/new/aq", false));
        assert_eq!(
            hooks["antiquota"]["PreInvocation"][0]["command"],
            "/new/aq hook pre-invocation"
        );
        let mut fresh = json!(null);
        assert!(install_hook_entry(&mut fresh, "/bin/aq", false));
        assert_eq!(fresh["antiquota"]["enabled"], true);
    }
}
