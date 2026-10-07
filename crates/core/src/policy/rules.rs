//! Owns permission rule lists, command and MCP rule matching, and safe permanent rules.
use super::shell::{command_parts, loose_parts};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use ts_rs::TS;

/// antiquota's own allow and deny lists, stored in the own stack's agy settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
pub struct Permissions {
    pub allow: Vec<String>,
    pub deny: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleVerdict {
    Allow,
    Deny,
}

/// Flags that turn an otherwise harmless, allowed command into one that deletes, overwrites or
/// runs other programs: an allow rule never covers them, so they still need a decision.
static RISKY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"^find\b.*\s-(exec|execdir|ok|okdir|delete|fprint\w*)\b",
        r"|^git\s+push\b.*\s(--force\S*|-f|--delete|--mirror|-d)\b|^git\s+push\b.*\s:\S",
        r"|^sed\b.*\s-i",
        r"|^git\s+(reset\s+--hard|checkout\s+--\s|stash\s+(drop|clear))\b",
        r"|^git\s+clean\b.*\s(-\w*f\w*|--force)\b",
        r"|^git\s+branch\b.*\s(-\w*[Df]\w*|--force)\b",
    ))
    .expect("valid risky-flag pattern")
});

/// Commands whose second word names what they do, so "Always allow" covers two words.
const TWO_WORD: [&str; 23] = [
    "npm", "npx", "pnpm", "yarn", "bun", "git", "cargo", "go", "uv", "pip", "pip3", "python",
    "python3", "node", "make", "docker", "kubectl", "gh", "brew", "poetry", "dotnet", "mvn",
    "gradle",
];

/// Programs that run whatever program or code they are given. A permanent allow rule for them
/// would allow anything, so they are only ever approved one call at a time.
const RUNNERS: [&str; 41] = [
    "sh",
    "bash",
    "zsh",
    "fish",
    "dash",
    "ksh",
    "csh",
    "tcsh",
    "pwsh",
    "powershell",
    "cmd",
    "env",
    "xargs",
    "awk",
    "gawk",
    "mawk",
    "nawk",
    "eval",
    "exec",
    "sudo",
    "doas",
    "su",
    "nohup",
    "timeout",
    "time",
    "nice",
    "ionice",
    "watch",
    "parallel",
    "script",
    "expect",
    "osascript",
    "ssh",
    "command",
    "builtin",
    "source",
    ".",
    "curl",
    "wget",
    "nc",
    "socat",
];

/// Interpreters and package runners: a rule must name what they run, not just the program.
const INTERPRETERS: [&str; 15] = [
    "python", "python3", "node", "deno", "bun", "ruby", "perl", "php", "lua", "npx", "bunx",
    "pnpx", "uvx", "pipx", "dotnet",
];

fn command_rules(rules: &[String]) -> Vec<&str> {
    rules
        .iter()
        .filter_map(|r| r.strip_prefix("command(")?.strip_suffix(')'))
        .map(str::trim)
        .collect()
}

fn hit(part: &str, rules: &[&str]) -> bool {
    rules.iter().any(|r| {
        *r == "*"
            || part == *r
            || part
                .strip_prefix(r)
                .is_some_and(|rest| rest.starts_with(' '))
    })
}

/// Whether one command uses a flag that an allow rule never covers.
pub fn is_risky(part: &str) -> bool {
    RISKY.is_match(part)
}

/// Allow or deny when antiquota's command rules decide a shell line; `None` means ask.
/// Denies win, including over a line that cannot be parsed. A line is allowed only when every
/// command in it is allowed and none uses a risky flag.
pub fn command_verdict(line: &str, permissions: &Permissions) -> Option<RuleVerdict> {
    let allow = command_rules(&permissions.allow);
    let deny = command_rules(&permissions.deny);
    if loose_parts(line).iter().any(|p| hit(p, &deny)) {
        return Some(RuleVerdict::Deny);
    }
    let parts = command_parts(line).filter(|parts| !parts.is_empty())?;
    if parts.iter().any(|p| hit(p, &deny)) {
        return Some(RuleVerdict::Deny);
    }
    parts
        .iter()
        .all(|p| hit(p, &allow) && !is_risky(p))
        .then_some(RuleVerdict::Allow)
}

/// The MCP rule a call would match, `mcp(server/tool)`, or empty without a server name.
pub fn mcp_rule(args: &serde_json::Value) -> String {
    let text = |a: &str, b: &str| {
        args.get(a)
            .and_then(serde_json::Value::as_str)
            .filter(|s| !s.is_empty())
            .or_else(|| args.get(b).and_then(serde_json::Value::as_str))
            .unwrap_or("")
            .to_owned()
    };
    let server = text("ServerName", "server");
    let tool = text("ToolName", "tool");
    if server.is_empty() {
        String::new()
    } else {
        format!("mcp({server}/{tool})")
    }
}

/// Exact and server-wide MCP denies win; exact, server-wide and global allows allow.
pub fn mcp_verdict(rule: &str, permissions: &Permissions) -> Option<RuleVerdict> {
    let server = rule
        .strip_prefix("mcp(")
        .and_then(|r| r.split('/').next())
        .unwrap_or("");
    let wide = format!("mcp({server}/*)");
    let has = |list: &[String], item: &str| list.iter().any(|r| r == item);
    if has(&permissions.deny, rule) || has(&permissions.deny, &wide) {
        Some(RuleVerdict::Deny)
    } else if has(&permissions.allow, rule)
        || has(&permissions.allow, &wide)
        || has(&permissions.allow, "mcp(*)")
    {
        Some(RuleVerdict::Allow)
    } else {
        None
    }
}

/// The rule "Always allow" would add for a command: two words for package, VCS, runtime and
/// build tools whose second word is not a flag, otherwise one. Empty when such a rule would
/// allow arbitrary programs, so the user approves those calls one at a time.
pub fn suggest_rule(line: &str) -> String {
    let words: Vec<&str> = line.split_whitespace().collect();
    let Some(first) = words.first() else {
        return String::new();
    };
    let n = if TWO_WORD.contains(first) && words.len() > 1 && !words[1].starts_with('-') {
        2
    } else {
        1
    };
    let rule = format!("command({})", words[..n].join(" "));
    if permanent_rule_error(&rule).is_some() {
        String::new()
    } else {
        rule
    }
}

/// Why a permanent allow rule is refused, or `None` when it is safe to keep. Rules that would
/// let agy run arbitrary programs (shells, `env`, `xargs`, `awk`, interpreters without a script,
/// network fetchers, `command(*)`) are refused at every place rules are written.
pub fn permanent_rule_error(rule: &str) -> Option<String> {
    let rule = rule.trim();
    let body = rule
        .strip_prefix("command(")
        .and_then(|r| r.strip_suffix(')'))?;
    let words: Vec<&str> = body.split_whitespace().collect();
    let refused = match words.as_slice() {
        [] => true,
        ["*", ..] => true,
        [first, ..] if RUNNERS.contains(first) => true,
        [first] if INTERPRETERS.contains(first) => true,
        [first, second, ..] if INTERPRETERS.contains(first) && second.starts_with('-') => true,
        _ => false,
    };
    refused.then(|| {
        format!(
            "{rule} would let agy run arbitrary programs; approve those calls one at a time instead"
        )
    })
}

/// Validates one submitted rule list: nonempty strings shorter than 300 characters, kept in
/// order without duplicates. Allow lists also refuse rules for arbitrary program runners.
pub fn clean_rules(key: &str, rules: &serde_json::Value) -> Result<Vec<String>, String> {
    let invalid = || format!("{key} must be a list of rule strings");
    let list = rules.as_array().ok_or_else(invalid)?;
    let mut out: Vec<String> = vec![];
    for rule in list {
        let text = rule.as_str().ok_or_else(invalid)?;
        let length = text.chars().count();
        if length == 0 || length >= 300 {
            return Err(invalid());
        }
        if key == "allow"
            && let Some(error) = permanent_rule_error(text)
        {
            return Err(error);
        }
        if !out.iter().any(|r| r == text) {
            out.push(text.to_owned());
        }
    }
    Ok(out)
}
