//! Owns interactive slash commands; all changes use the existing authenticated engine API.
use crate::{engine::Engine, terminal::Terminal};
use antiquota_core::Session;
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
    "/usage",
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
        "{} · {} · {} · {}\n{}\n{} turns · {} input / {} output / {} thinking tokens · context {} · account {}",
        meta.title,
        meta.id,
        meta.model,
        mode.as_str().unwrap(),
        meta.cwd,
        meta.turns,
        meta.usage.input,
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
            terminal.line("/sessions · /resume ID · /new · /fork · /rename TITLE\n/model [ID] · /mode [default|accept-edits|plan] · /status\n/accounts · /usage · /diff · /compact · /exit\nEnter sends · Ctrl+J or \\ Enter adds a line · Up/Down history\nTab completes a unique command · Ctrl+C/Escape stops a turn\nTool approval: y allows once; n, Enter or Escape denies.\nHistory stays in memory; resume restores this session's user prompts.")?;
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
            return Ok(Action::Switch(Box::new(
                engine.saved(session_id(arg)?).await?.meta,
            )));
        }
        "/new" | "/fork" => {
            no_args()?;
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
}
