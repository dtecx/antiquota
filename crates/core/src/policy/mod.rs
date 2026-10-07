//! Owns which agy tool calls run on their own, which are refused, and which wait for the user.
//!
//! Headless agy ignores its own allow rules, so antiquota runs it with skip-permissions while
//! its PreToolUse hook is on, and this module is the permission layer instead:
//! reading and bookkeeping are allowed; shell commands and MCP tools follow antiquota's rules;
//! file edits follow the session mode; side effects outside the project always ask.
mod rules;
pub mod shell;
#[cfg(test)]
mod tests;
use crate::Mode;
pub use rules::*;
use serde_json::Value;

pub const COMMAND_TOOLS: [&str; 7] = [
    "run_command",
    "shell_exec",
    "send_command_input",
    "execute_notebook",
    "run_extension_code",
    "restart_dev_server",
    "git_commit",
];
pub const WRITE_TOOLS: [&str; 9] = [
    "write_to_file",
    "replace_file_content",
    "multi_replace_file_content",
    "sed_file",
    "file_change",
    "edit_notebook",
    "move",
    "delete_directory",
    "write_blob",
];
pub const MCP_TOOLS: [&str; 2] = ["call_mcp_tool", "mcp_tool"];
pub const ASK_TOOLS: [&str; 8] = [
    "deploy_firebase",
    "set_up_firebase",
    "cloud_sql_execute_sql",
    "cloud_sql_update_schema",
    "set_up_cloud_sql",
    "install_applet_package",
    "install_applet_dependencies",
    "post_pr_review",
];
pub const DECLINED: &str =
    "The user declined this. Do not retry it; continue another way, or explain what you need.";
pub const STOPPED: &str = "The user stopped this session.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Allow,
    Deny,
    Ask,
}
impl Verdict {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
            Self::Ask => "ask",
        }
    }
}

/// One permission decision: the verdict, the reason agy hears, and the rule "Always allow"
/// would add (empty when there is none or it would be unsafe).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    pub verdict: Verdict,
    pub reason: String,
    pub rule: String,
}
impl Decision {
    fn new(verdict: Verdict, reason: &str, rule: String) -> Self {
        Self {
            verdict,
            reason: reason.into(),
            rule,
        }
    }
}

/// The command line a shell tool call runs: its CommandLine, or `git commit` for git_commit.
pub fn command_line(tool: &str, args: &Value) -> String {
    let given = match args.get("CommandLine") {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Number(n)) if n.as_f64() != Some(0.0) => n.to_string(),
        Some(Value::Bool(true)) => "True".into(),
        Some(value @ (Value::Array(_) | Value::Object(_)))
            if value.as_array().is_none_or(|a| !a.is_empty())
                && value.as_object().is_none_or(|o| !o.is_empty()) =>
        {
            value.to_string()
        }
        _ => String::new(),
    };
    if given.is_empty() && tool == "git_commit" {
        "git commit".into()
    } else {
        given
    }
}

pub fn decide(mode: Mode, tool: &str, args: &Value, permissions: &Permissions) -> Decision {
    if mode == Mode::Bypass {
        return Decision::new(Verdict::Allow, "", String::new());
    }
    if tool == "run_command" || tool == "git_commit" {
        let line = command_line(tool, args);
        return match command_verdict(&line, permissions) {
            Some(RuleVerdict::Deny) => Decision::new(
                Verdict::Deny,
                "A deny rule in antiquota's permissions blocks this command.",
                String::new(),
            ),
            Some(RuleVerdict::Allow) => Decision::new(Verdict::Allow, "", String::new()),
            None => Decision::new(Verdict::Ask, "", suggest_rule(&line)),
        };
    }
    if COMMAND_TOOLS.contains(&tool) {
        return Decision::new(Verdict::Ask, "", String::new());
    }
    if WRITE_TOOLS.contains(&tool) {
        return match mode {
            Mode::AcceptEdits => Decision::new(Verdict::Allow, "", String::new()),
            Mode::Plan => Decision::new(
                Verdict::Deny,
                "Plan mode: describe the change instead of making it.",
                String::new(),
            ),
            _ => Decision::new(Verdict::Ask, "", String::new()),
        };
    }
    if MCP_TOOLS.contains(&tool) {
        let rule = mcp_rule(args);
        let verdict = if rule.is_empty() {
            None
        } else {
            mcp_verdict(&rule, permissions)
        };
        return match verdict {
            Some(RuleVerdict::Deny) => Decision::new(
                Verdict::Deny,
                "A deny rule in antiquota's permissions blocks this MCP tool.",
                String::new(),
            ),
            Some(RuleVerdict::Allow) => Decision::new(Verdict::Allow, "", String::new()),
            None => Decision::new(Verdict::Ask, "", rule),
        };
    }
    if ASK_TOOLS.contains(&tool) {
        return Decision::new(Verdict::Ask, "", String::new());
    }
    Decision::new(Verdict::Allow, "", String::new())
}
