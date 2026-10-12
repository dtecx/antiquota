//! Owns interactive slash commands; all changes use the existing authenticated engine API.
use crate::{engine::Engine, terminal::Terminal};
use antiquota_core::{Attachment, RoutingEntry, Session};
use antiquota_server::api::StateResponse;
use anyhow::{Context, bail};
use reqwest::Method;
use serde_json::json;

pub const NAMES: &[&str] = &[
    "/help",
    "/status",
    "/sessions",
    "/resume",
    "/new",
    "/fork",
    "/model",
    "/mode",
    "/rename",
    "/accounts",
    "/routing",
    "/usage",
    "/attach",
    "/attachments",
    "/detach",
    "/export",
    "/diff",
    "/compact",
    "/exit",
];
pub enum Action {
    Stay,
    Switch(Box<Session>),
    Exit,
}

pub fn session_id(id: &str) -> anyhow::Result<&str> {
    anyhow::ensure!(
        !id.is_empty()
            && id.len() <= 100
            && id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_')),
        "use an exact session id from /sessions"
    );
    Ok(id)
}
pub fn status(meta: &Session) -> String {
    let mode = serde_json::to_value(meta.mode).unwrap();
    format!(
        "{} · {} · {} · {}\n{}\n{} turns · {} input ({} cached) / {} output / {} thinking tokens · context {} · account {}",
        meta.title,
        meta.id,
        meta.model,
        mode.as_str().unwrap(),
        meta.cwd,
        meta.turns,
        meta.usage.input + meta.usage.cache,
        meta.usage.cache,
        meta.usage.output,
        meta.usage.thinking,
        meta.context,
        meta.last_slot.as_deref().unwrap_or("automatic")
    )
}
pub async fn run(
    engine: &Engine,
    meta: &Session,
    terminal: &Terminal,
    line: &str,
    pending: &mut Vec<Attachment>,
) -> anyhow::Result<Action> {
    let (name, arg) = line
        .trim()
        .split_once(char::is_whitespace)
        .unwrap_or((line.trim(), ""));
    let arg = arg.trim();
    let path = format!("/api/sessions/{}", session_id(&meta.id)?);
    let no_args = || -> anyhow::Result<()> {
        anyhow::ensure!(arg.is_empty(), "{name} takes no arguments");
        Ok(())
    };
    match name {
        "/help" => {
            no_args()?;
            terminal.line("/sessions · /resume ID · /new · /fork · /rename TITLE\n/model [ID] · /mode [default|accept-edits|plan] · /status\n/accounts [check|pause|resume SLOT] · /routing [1-200] · /usage\n/attach PATH · /attachments · /detach N|all · /export PATH\n/diff · /compact · /exit\nEnter sends · Ctrl+J or \\ Enter adds a line · Up/Down history\nFiles: paths use the session folder; Enter can send only attachments.\nExport: full saved session JSON, including tools, with no overwrite.\nTab completes a unique command · Ctrl+C/Escape stops a turn\nTool approval: y allows once; n, Enter or Escape denies.\nHistory stays in memory; resume restores this session's user prompts.")?;
        }
        "/status" => {
            no_args()?;
            terminal.line(&status(meta))?;
        }
        "/sessions" => {
            no_args()?;
            let mut state: StateResponse =
                serde_json::from_value(engine.call(Method::GET, "/api/state", json!({})).await?)?;
            state
                .sessions
                .sort_by(|a, b| b.updated.total_cmp(&a.updated));
            for s in state.sessions {
                terminal.line(&format!(
                    "{}  {}  {}  {}{}",
                    s.id,
                    s.status,
                    s.title,
                    s.cwd,
                    if s.archived { " [archived]" } else { "" }
                ))?;
            }
        }
        "/resume" => {
            no_pending(pending)?;
            return Ok(Action::Switch(Box::new(
                engine.saved(session_id(arg)?).await?.meta,
            )));
        }
        "/new" | "/fork" => {
            no_args()?;
            no_pending(pending)?;
            let (target, body) = if name == "/new" {
                (
                    "/api/sessions".to_owned(),
                    json!({"cwd":meta.cwd,"model":meta.model,"mode":meta.mode}),
                )
            } else {
                (format!("{path}/fork"), json!({}))
            };
            let session = serde_json::from_value(engine.call(Method::POST, &target, body).await?)?;
            return Ok(Action::Switch(Box::new(session)));
        }
        "/model" if arg.is_empty() => {
            let state: StateResponse =
                serde_json::from_value(engine.call(Method::GET, "/api/state", json!({})).await?)?;
            terminal.line(&format!("Current: {}", meta.model))?;
            for m in state.models {
                terminal.line(&format!("{}  {}", m.id, m.label))?;
            }
        }
        "/mode" if arg.is_empty() => terminal.line("default · accept-edits · plan")?,
        "/model" | "/mode" | "/rename" => {
            anyhow::ensure!(!arg.is_empty(), "{name} needs a value");
            if name == "/mode" {
                anyhow::ensure!(
                    ["default", "accept-edits", "plan"].contains(&arg),
                    "mode must be default, accept-edits or plan"
                );
            }
            let key = if name == "/rename" {
                "title"
            } else {
                &name[1..]
            };
            engine.call(Method::PATCH, &path, json!({key:arg})).await?;
        }
        "/attach" | "/attachments" | "/detach" | "/export" => {
            crate::context::run(engine, meta, terminal, name, arg, pending).await?;
        }
        "/accounts" if !arg.is_empty() => {
            let (action, slot) = arg
                .split_once(char::is_whitespace)
                .context("use /accounts check|pause|resume SLOT")?;
            let (target, body) = account_action(action, slot.trim())?;
            Engine::execute(
                engine
                    .request(Method::POST, &target)
                    .timeout(std::time::Duration::from_secs(200))
                    .json(&body),
            )
            .await?;
            terminal.line(&format!("Account {slot}: {action} complete."))?;
        }
        "/routing" => {
            let n = routing_count(arg)?;
            let rows: Vec<RoutingEntry> = serde_json::from_value(
                engine
                    .call(Method::GET, &format!("/api/routing?n={n}"), json!({}))
                    .await?,
            )?;
            if rows.is_empty() {
                terminal.line("No recorded routing decisions.")?;
            }
            for row in rows.iter().rev() {
                terminal.line(&routing_line(row))?;
            }
        }
        "/accounts" => {
            no_args()?;
            let state: StateResponse =
                serde_json::from_value(engine.call(Method::GET, "/api/state", json!({})).await?)?;
            if state.accounts.is_empty() {
                terminal.line("No account slots. Use antiquota login to sign in.")?;
            }
            for a in state.accounts {
                terminal.line(&format!(
                    "{} · {} · {} tokens today · {} active cooldowns",
                    a.slot,
                    if a.busy {
                        "busy"
                    } else if a.paused {
                        "paused"
                    } else if a.verify.is_some() {
                        "verification required"
                    } else {
                        "idle"
                    },
                    a.today.input.unwrap_or(0)
                        + a.today.output.unwrap_or(0)
                        + a.today.thinking.unwrap_or(0)
                        + a.today.cache.unwrap_or(0)
                        + a.today.cache.unwrap_or(0),
                    a.walls.values().filter(|until| **until > 0.0).count()
                ))?;
                for (bucket, models) in a.quota.0 {
                    for (model, (fraction, _)) in models {
                        terminal.line(&format!(
                            "  {bucket} · {model}: {:.0}% quota",
                            fraction * 100.0
                        ))?;
                    }
                }
            }
        }
        "/usage" => {
            no_args()?;
            let usage = engine
                .call(Method::GET, "/api/usage?detail=1", json!({}))
                .await?;
            terminal.line(
                &serde_json::to_string_pretty(&usage["detail"]).context("unreadable usage")?,
            )?;
        }
        "/diff" => {
            no_args()?;
            let diff = engine
                .call(Method::GET, &format!("{path}/git/diff"), json!({}))
                .await?;
            terminal.line(
                diff["diff"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .unwrap_or("No working-tree diff."),
            )?;
        }
        "/compact" => {
            no_args()?;
            engine
                .call(Method::POST, &format!("{path}/compact"), json!({}))
                .await?;
            terminal.line("Context compacted; the saved transcript remains available.")?;
        }
        "/exit" => {
            no_args()?;
            return Ok(Action::Exit);
        }
        _ => bail!("unknown command {name}; use /help"),
    }
    Ok(Action::Stay)
}

fn no_pending(pending: &[Attachment]) -> anyhow::Result<()> {
    anyhow::ensure!(
        pending.is_empty(),
        "send the staged files or /detach all before switching sessions"
    );
    Ok(())
}

fn account_action(action: &str, slot: &str) -> anyhow::Result<(String, serde_json::Value)> {
    antiquota_core::validate_account_name(slot)?;
    let (tail, body) = match action {
        "check" => ("check", json!({})),
        "pause" | "resume" => ("pause", json!({"on": action == "pause"})),
        _ => bail!("use /accounts check|pause|resume SLOT"),
    };
    Ok((
        format!("/api/accounts/{}/{tail}", crate::engine::component(slot)),
        body,
    ))
}

fn routing_count(arg: &str) -> anyhow::Result<usize> {
    if arg.is_empty() {
        return Ok(20);
    }
    arg.parse::<usize>()
        .ok()
        .filter(|n| (1..=antiquota_core::ROUTING_READ).contains(n))
        .context("routing count must be between 1 and 200")
}

fn routing_line(row: &RoutingEntry) -> String {
    let time = chrono::DateTime::from_timestamp(row.ts as i64, 0)
        .map(|t| t.to_rfc3339())
        .unwrap_or_else(|| "unknown time".into());
    let mut parts = vec![time, row.event.clone()];
    for (label, value) in [
        ("account", &row.slot),
        ("pool", &row.bucket),
        ("from", &row.previous),
        ("session", &row.session),
        ("reason", &row.why),
    ] {
        if let Some(value) = value {
            parts.push(format!("{label} {value}"));
        }
    }
    if let Some(wait) = row.wait {
        parts.push(format!("wait {wait:.0}s"));
    }
    if let Some(until) = row.until {
        parts.push(format!("until {until:.0}"));
    }
    parts.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn session_paths_cannot_escape_or_inject_a_query() {
        for id in [
            "",
            "../secret",
            "x?token=x",
            "x/y",
            "x%2fy",
            "x\\y",
            "x\u{1b}",
        ] {
            assert!(session_id(id).is_err());
        }
        assert_eq!(session_id("session-123_abc").unwrap(), "session-123_abc");
    }
    #[test]
    fn account_actions_and_routing_counts_are_bounded() {
        assert_eq!(
            account_action("pause", "a@example.com").unwrap(),
            (
                "/api/accounts/a@example.com/pause".to_owned(),
                json!({"on": true})
            )
        );
        assert_eq!(
            account_action("resume", "a@example.com").unwrap().1,
            json!({"on": false})
        );
        assert!(account_action("remove", "a@example.com").is_err());
        assert!(account_action("check", "../x").is_err());
        assert_eq!(routing_count("").unwrap(), 20);
        assert_eq!(routing_count("200").unwrap(), 200);
        for bad in ["0", "201", "x", "-1"] {
            assert!(routing_count(bad).is_err());
        }
    }
}
