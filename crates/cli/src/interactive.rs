//! Owns the interactive shared-session loop, streaming, explicit approvals and turn cancellation.
use crate::{
    commands::{self, Action},
    engine::{Engine, interactive_engine},
    follow::Follow,
    terminal::{Editor, Input, Terminal},
};
use antiquota_core::{Event as RecordEvent, Session};
use anyhow::Context;
use clap::Args;
use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind, KeyModifiers};
use futures_util::StreamExt;
use reqwest::Method;
use serde_json::json;
use std::time::Duration;

#[derive(Args, Debug)]
pub struct InteractiveArgs {
    /// Open an interactive shared session in this terminal.
    #[arg(short = 'i', long)]
    pub interactive: bool,
    /// Resume an exact session id (list sessions with /sessions).
    #[arg(long, requires = "interactive", conflicts_with_all = ["cwd", "model", "mode"])]
    resume: Option<String>,
    #[arg(long, requires = "interactive")]
    cwd: Option<String>,
    #[arg(long, requires = "interactive")]
    model: Option<String>,
    #[arg(long, requires = "interactive", value_parser = ["default", "accept-edits", "plan"])]
    mode: Option<String>,
}

pub async fn run(args: InteractiveArgs) -> anyhow::Result<()> {
    if let Some(id) = &args.resume {
        commands::session_id(id)?;
    }
    let terminal = Terminal::open()?;
    terminal.heading(&format!("◌ antiquota {}", env!("CARGO_PKG_VERSION")))?;
    terminal.line("Shared sessions · /help for commands · Ctrl+D exits\n")?;
    let engine = interactive_engine().await?;
    workspace(&engine, &terminal, args).await
}

async fn workspace(
    engine: &Engine,
    terminal: &Terminal,
    args: InteractiveArgs,
) -> anyhow::Result<()> {
    let mut meta: Session = if let Some(id) = args.resume {
        engine.saved(&id).await?.meta
    } else {
        let cwd = std::fs::canonicalize(args.cwd.as_deref().unwrap_or("."))
            .context("the working folder is unavailable")?;
        let mut body = json!({"cwd":cwd});
        if let Some(model) = args.model {
            body["model"] = json!(model);
        }
        if let Some(mode) = args.mode {
            body["mode"] = json!(mode);
        }
        serde_json::from_value(engine.call(Method::POST, "/api/sessions", body).await?)?
    };
    let mut keys = EventStream::new();
    let mut editor = Editor::default();
    let mut follow = load(engine, terminal, &mut editor, &meta.id).await?;
    loop {
        if follow.meta.status != "idle" && busy(engine, terminal, &mut keys, &mut follow).await? {
            break;
        }
        meta = engine.saved(&meta.id).await?.meta;
        terminal.heading(&format!(
            "{} · {} · account {}",
            meta.model,
            serde_json::to_value(meta.mode)?
                .as_str()
                .unwrap_or("default"),
            meta.last_slot.as_deref().unwrap_or("automatic")
        ))?;
        terminal.line(&format!("{} · {}", meta.cwd, meta.id))?;
        terminal.prompt(&editor, "❯")?;
        let line = loop {
            let event = keys.next().await.context("terminal input closed")??;
            match editor.event(event) {
                Some(Input::Line(line)) => break Some(line),
                Some(Input::Exit) => break None,
                Some(Input::Cancel) => terminal.line("\nDraft cleared; Ctrl+D exits.")?,
                None => {}
            }
            terminal.prompt(&editor, "❯")?;
        };
        terminal.clear_input()?;
        let Some(line) = line else {
            break;
        };
        if line.trim().is_empty() {
            continue;
        }
        terminal.heading("you")?;
        terminal.line(&line)?;
        if line.starts_with('/') {
            match commands::run(engine, &meta, terminal, &line).await {
                Ok(Action::Exit) => break,
                Ok(Action::Switch(session)) => {
                    meta = *session;
                    editor = Editor::default();
                    follow = load(engine, terminal, &mut editor, &meta.id).await?;
                }
                Ok(Action::Stay) => {}
                Err(error) => terminal.line(&format!("antiquota: {error}"))?,
            }
            continue;
        }
        // Open before sending, then confirm completion from saved state to cover short turns.
        let mut stream = Some(engine.events().await?);
        match engine
            .call(
                Method::POST,
                &format!("/api/sessions/{}/send", meta.id),
                json!({"text":line}),
            )
            .await
        {
            Ok(_) => {
                follow.echoed_user = Some(line);
                follow.accepted();
                terminal.heading("assistant · Ctrl+C or Escape stops")?;
                if follow_stream(engine, terminal, &mut keys, &mut follow, &mut stream).await? {
                    break;
                }
            }
            Err(error) => terminal.line(&format!("antiquota: {error}"))?,
        }
    }
    terminal.line(&format!(
        "\nSaved session {}. Resume with antiquota -i --resume {}",
        meta.id, meta.id
    ))?;
    Ok(())
}

async fn load(
    engine: &Engine,
    terminal: &Terminal,
    editor: &mut Editor,
    id: &str,
) -> anyhow::Result<Follow> {
    let saved = engine.saved(commands::session_id(id)?).await?;
    terminal.line(&commands::status(&saved.meta))?;
    let mut follow = Follow::new(id, &saved.meta.cwd);
    follow.interactive = true;
    let mut output = vec![];
    follow.catch_up(&mut output, &saved.meta, &saved.events);
    terminal.write(&String::from_utf8_lossy(&output))?;
    for record in &saved.events {
        if let RecordEvent::User { text, kind, .. } = &record.event
            && kind == "user"
        {
            editor.seed(text);
        }
    }
    Ok(follow)
}

async fn busy(
    engine: &Engine,
    terminal: &Terminal,
    keys: &mut EventStream,
    follow: &mut Follow,
) -> anyhow::Result<bool> {
    let mut stream = Some(engine.events().await?);
    // A resumed session may finish while the event stream is opening.
    follow.accepted();
    follow_stream(engine, terminal, keys, follow, &mut stream).await
}

async fn follow_stream(
    engine: &Engine,
    terminal: &Terminal,
    keys: &mut EventStream,
    follow: &mut Follow,
    stream: &mut Option<reqwest::Response>,
) -> anyhow::Result<bool> {
    let id = follow.meta.id.clone();
    let mut buffer = vec![];
    let mut editor = Editor::default();
    let mut approval_id: Option<String> = None;
    let mut exit = false;
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    loop {
        let mut output = vec![];
        let mut warnings = vec![];
        tokio::select! {
            event = keys.next() => {
                let event = event.context("terminal input closed")??;
                if let Event::Key(key) = event && key.kind != KeyEventKind::Release &&
                    ((key.code == KeyCode::Char('c') || key.code == KeyCode::Char('d')) && key.modifiers.contains(KeyModifiers::CONTROL)
                    || key.code == KeyCode::Esc && approval_id.is_none()) {
                    exit |= key.code == KeyCode::Char('d');
                    terminal.clear_input()?;
                    engine.call(Method::POST, &format!("/api/sessions/{id}/stop"), json!({})).await?;
                    terminal.line("\nStopping…")?;
                } else if let Some(aid) = &approval_id {
                    let decision = match editor.event(event) {
                        Some(Input::Line(line)) => match line.trim().to_ascii_lowercase().as_str() {
                            "y" | "yes" => Some("allow"), "" | "n" | "no" => Some("deny"),
                            _ => { terminal.line("\nUse y to allow once or n to deny.")?; None }
                        },
                        Some(Input::Cancel | Input::Exit) => Some("deny"), None => None,
                    };
                    if let Some(decision) = decision {
                        terminal.clear_input()?;
                        let answer = engine.call(Method::POST, &format!("/api/approvals/{aid}/answer"), json!({"decision":decision,"always":false})).await;
                        terminal.line(&format!("\n{}", match answer { Ok(value) => approval_message(&value)?, Err(e) => format!("approval no longer available: {e}") }))?;
                        follow.approvals.remove(aid);
                        approval_id = None;
                        editor.reset();
                    }
                }
            }
            chunk = async { match stream { Some(s) => s.chunk().await, None => std::future::pending().await } } => {
                match chunk {
                    Ok(Some(chunk)) => { buffer.extend_from_slice(&chunk); follow.frames(&mut output, &mut warnings, &mut buffer); }
                    _ => { *stream = None; buffer.clear(); terminal.line("\nConnection interrupted; reconnecting from the saved session…")?; }
                }
            }
            _ = tick.tick() => {
                if stream.is_none() {
                    match engine.events().await { Ok(s) => *stream = Some(s), Err(_) => continue }
                }
                match engine.saved(&id).await {
                    Ok(saved) => {
                        let done = follow.catch_up(&mut output, &saved.meta, &saved.events);
                        if done {
                            terminal.clear_input()?;
                            terminal.write(&String::from_utf8_lossy(&output))?;
                            terminal.line("\n")?;
                            return Ok(exit);
                        }
                    }
                    Err(error) => { terminal.line(&format!("\nSaved session unavailable: {error}"))?; continue; }
                }
            }
        }
        if follow.deleted {
            anyhow::bail!("the session was deleted in another client");
        }
        if !output.is_empty() || !warnings.is_empty() {
            if approval_id.is_some() {
                terminal.clear_input()?;
            }
            terminal.write(&String::from_utf8_lossy(&output))?;
            terminal.write(&String::from_utf8_lossy(&warnings))?;
        }
        let next = follow.approvals.keys().next().cloned();
        if next != approval_id {
            if approval_id.is_some() {
                terminal.clear_input()?;
                terminal.line("\nApproval resolved in another client or expired.")?;
            }
            editor.reset();
            approval_id = next;
            if let Some(aid) = &approval_id {
                let a = &follow.approvals[aid];
                terminal.heading(&format!("\nPermission · {}", a.tool))?;
                terminal.line(&serde_json::to_string_pretty(&a.args)?)?;
                terminal.line("Allow this call once? y = allow · Enter/n/Escape = deny")?;
            }
        }
        if approval_id.is_some() {
            terminal.prompt(&editor, "[y/N]")?;
        }
    }
}

/// Another client may have answered first; display the engine's decision, never the request.
fn approval_message(reply: &serde_json::Value) -> anyhow::Result<String> {
    let decision = reply["decision"]
        .as_str()
        .filter(|value| matches!(*value, "allow" | "deny"))
        .context("the engine returned no approval decision; check the app")?;
    Ok(format!("Engine decision: {decision}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn approval_messages_use_the_engine_decision_and_reject_missing_results() {
        assert_eq!(
            approval_message(&json!({"decision":"allow"})).unwrap(),
            "Engine decision: allow"
        );
        assert_eq!(
            approval_message(&json!({"decision":"deny","reason":"answered elsewhere"})).unwrap(),
            "Engine decision: deny"
        );
        for value in [
            json!({}),
            json!({"pending":"request"}),
            json!({"decision":"unknown"}),
        ] {
            assert!(approval_message(&value).is_err());
        }
    }
}
