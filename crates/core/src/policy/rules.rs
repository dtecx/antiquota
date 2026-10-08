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

/// Identify programs without broadening the actual path prefix stored in a rule.
fn program_identity(program: &str) -> String {
    let name = program.rsplit(['/', '\\']).next().unwrap_or(program);
    let lower = name.to_ascii_lowercase();
    let name = lower.strip_suffix(".exe").unwrap_or(&lower);
    if name == "nodejs" {
        return "node".into();
    }
    if let Some(version) = name.strip_prefix("python")
        && !version.is_empty()
        && version.starts_with(|c: char| c.is_ascii_digit())
        && version.chars().all(|c| c.is_ascii_digit() || c == '.')
    {
        return "python".into();
    }
    name.into()
}

/// Read one executable token with the existing shell parser. Keeping its quoted boundary
/// separately matters for paths containing spaces; the parser's joined command loses it.
fn single_command(line: &str) -> Option<(String, String, String)> {
    let parts = command_parts(line)?;
    if parts.len() != 1 {
        return None;
    }
    let line = line.trim_start();
    let mut quote = None;
    let mut escaped = false;
    let mut end = line.len();
    for (at, c) in line.char_indices() {
        if escaped {
            escaped = false;
        } else if c == '\\' && quote != Some('\'') {
            escaped = true;
        } else if quote == Some(c) {
            quote = None;
        } else if quote.is_none() {
            if c == '\'' || c == '"' {
                quote = Some(c);
            } else if " \t\r\n();<>|&".contains(c) {
                end = at;
                break;
            }
        }
    }
    let programs = command_parts(&line[..end])?;
    let [program] = programs.as_slice() else {
        return None;
    };
    // Shell assignments and expansions cannot identify a concrete executable safely.
    if program.is_empty() || program.contains(['=', '$', '*', '?', '[', ']', '{', '}']) {
        return None;
    }
    let raw = &line[..end];
    // The shell parser treats bare backslashes as POSIX escapes. Also guard literal Windows
    // drive paths so that writing a runner rule never depends on the host's shell syntax.
    let identity = if raw.as_bytes().get(1) == Some(&b':') && raw.contains('\\') {
        program_identity(raw)
    } else {
        program_identity(program)
    };
    let tail = parts[0].strip_prefix(program)?.trim_start().to_owned();
    Some((program.clone(), tail, identity))
}

/// Whether one command uses a flag that an allow rule never covers.
pub fn is_risky(part: &str) -> bool {
    // Joined commands may contain a space inside their executable path. For those paths,
    // conservatively check each possible boundary until its basename identifies a risky tool.
    let path = part
        .split_whitespace()
        .next()
        .is_some_and(|p| p.contains(['/', '\\']));
    for (end, _) in part.char_indices().filter(|(_, c)| c.is_whitespace()) {
        let candidate = part[..end].split_whitespace().next_back().unwrap_or("");
        let identity = program_identity(candidate);
        if ["find", "git", "sed"].contains(&identity.as_str())
            && RISKY.is_match(&format!("{identity}{}", &part[end..]))
        {
            return true;
        }
        if !path {
            break;
        }
    }
    false
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
/// allow arbitrary programs or the call uses risky flags, so the user approves it one at a time.
pub fn suggest_rule(line: &str) -> String {
    let Some((program, tail, identity)) = single_command(line) else {
        return String::new();
    };
    // Rule matching uses joined words, so it cannot retain a quoted executable boundary.
    if program.chars().any(char::is_whitespace)
        || is_risky(&format!("{program} {tail}"))
        || permanent_rule_error(&format!("command({line})")).is_some()
    {
        return String::new();
    }
    let second = tail.split_whitespace().next();
    let prefix = match second {
        Some(second) if TWO_WORD.contains(&identity.as_str()) && !second.starts_with('-') => {
            format!("{program} {second}")
        }
        _ => program,
    };
    let rule = format!("command({prefix})");
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
    let refused = match single_command(body) {
        None => true,
        Some((_, tail, identity)) => {
            RUNNERS.contains(&identity.as_str())
                || (INTERPRETERS.contains(&identity.as_str())
                    && tail
                        .split_whitespace()
                        .next()
                        .is_none_or(|s| s.starts_with('-')))
        }
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
