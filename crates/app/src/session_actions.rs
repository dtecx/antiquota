//! Owns explicit transcript, conversation and project actions without an agent turn.
use crate::{AppError, Service, SessionState};
use antiquota_core::{
    Event, Session, SessionPatch,
    state_document::{document, handoff, tail},
};
use serde_json::{Value, json};
use std::sync::Arc;

impl Service {
    pub fn state_document(&self, id: &str) -> Result<String, AppError> {
        let (meta, records) = self.get(id)?;
        Ok(document(&meta, &records))
    }
    pub fn search(&self, query: &str) -> Result<Value, AppError> {
        let query = query.trim().to_lowercase();
        if query.chars().count() < 2 {
            return Ok(json!([]));
        }
        let mut hits = vec![];
        for meta in self.sessions()? {
            let (_, records) = self.get(&meta.id)?;
            for r in records.iter().rev() {
                let (text, who) = match &r.event {
                    Event::User { text, .. } => (text, "user"),
                    Event::Assistant { text, .. } => (text, "assistant"),
                    _ => continue,
                };
                // Count characters, not UTF-8 bytes, when taking the snippet.
                if let Some(at) = text.to_lowercase().find(&query) {
                    let i = text.to_lowercase()[..at].chars().count();
                    let snippet: String = text
                        .chars()
                        .skip(i.saturating_sub(60))
                        .take(if i > 60 { 160 } else { i + 100 })
                        .collect();
                    hits.push(json!({"id":meta.id,"title":meta.title,"cwd":meta.cwd,"ts":r.ts,"who":who,"snippet":format!("{}{}",if i>60{"…"}else{""},snippet.replace('\n'," "))}));
                    break;
                }
            }
        }
        hits.sort_by(|a, b| {
            b["ts"]
                .as_f64()
                .unwrap_or(0.0)
                .total_cmp(&a["ts"].as_f64().unwrap_or(0.0))
        });
        hits.truncate(40);
        Ok(json!(hits))
    }
    pub fn fork(&self, id: &str) -> Result<Session, AppError> {
        let (src, records) = self.get(id)?;
        let title = format!(
            "{} (fork)",
            if src.title.is_empty() {
                "Untitled"
            } else {
                &src.title
            }
        );
        let new = self.create(&src.cwd, &title, &src.model, src.mode)?;
        self.patch(
            &new.id,
            &SessionPatch {
                effort: Some(src.effort.clone()),
                ..Default::default()
            },
        )?;
        self.inherit_document(&new.id, &src, &records)?;
        Ok(self.get(&new.id)?.0)
    }
    pub fn inherit(&self, id: &str, source: &str) -> Result<(), AppError> {
        let (src, records) = self.get(source)?;
        self.inherit_document(id, &src, &records)
    }
    fn inherit_document(
        &self,
        id: &str,
        src: &Session,
        records: &[antiquota_core::EventRecord],
    ) -> Result<(), AppError> {
        let cell = self.cell(id)?;
        let mut state = cell
            .state
            .lock()
            .map_err(|_| AppError::Invalid("session unavailable".into()))?;
        Self::idle_action(&state, id)?;
        let note = format!(
            "You are continuing work from an earlier antiquota session. Its handoff state follows; check the current files yourself before relying on it.\n\n{}",
            document(src, records)
        );
        let mut next = state.meta.clone();
        add_note(&mut next.pending_note, &note);
        self.replace_meta(&mut state, next)?;
        self.append(
            &mut state,
            Event::Notice {
                kind: "info".into(),
                text: format!(
                    "Picks up from \"{}\" with the next message.",
                    if src.title.is_empty() {
                        &src.id
                    } else {
                        &src.title
                    }
                ),
                data: json!({"source":src.id}),
            },
        )
    }
    pub(crate) fn idle_action(state: &SessionState, id: &str) -> Result<(), AppError> {
        Self::active(state, id)?;
        if state.busy || !state.shells.is_empty() {
            return Err(AppError::Invalid("stop the running turn first".into()));
        }
        Ok(())
    }
    pub fn clear(&self, id: &str, compact: bool) -> Result<(), AppError> {
        let cell = self.cell(id)?;
        let mut state = cell
            .state
            .lock()
            .map_err(|_| AppError::Invalid("session unavailable".into()))?;
        Self::idle_action(&state, id)?;
        let mut next = state.meta.clone();
        next.conversation_id = None;
        next.context = 0;
        next.pending_note = if compact {
            handoff(&state.meta, &state.records, "(the next message follows)")
        } else {
            String::new()
        };
        self.replace_meta(&mut state, next)?;
        self.append(&mut state,Event::Notice{kind:"info".into(),text:if compact {"Compacted: the next message starts a fresh conversation from a summary of this one."}else{"New conversation: agy starts fresh from the next message."}.into(),data:json!({})})
    }
    pub fn rewind(&self, id: &str, turn: u64) -> Result<Vec<String>, AppError> {
        let cell = self.cell(id)?;
        let mut state = cell
            .state
            .lock()
            .map_err(|_| AppError::Invalid("session unavailable".into()))?;
        Self::idle_action(&state, id)?;
        let (files, outside) = self.store.rewind(id, turn, &state.meta.cwd)?;
        let mut next = state.meta.clone();
        next.pending_note = format!(
            "Files were rewound to how they were before turn {turn}: {}. Re-read them before editing.",
            files.join(", ")
        );
        self.replace_meta(&mut state, next)?;
        self.append(
            &mut state,
            Event::Notice {
                kind: "rewind".into(),
                text: format!(
                    "Rewound {} file{} to before turn {turn}.",
                    files.len(),
                    if files.len() > 1 { "s" } else { "" }
                ),
                data: json!({"files":files,"outside":outside,"turn":turn}),
            },
        )?;
        Ok(files)
    }
    pub async fn shell(self: &Arc<Self>, id: &str, cmd: &str) -> Result<Value, AppError> {
        if cmd.trim().is_empty() {
            return Err(AppError::Invalid("empty command".into()));
        }
        let cell = self.cell(id)?;
        let call_id = uuid::Uuid::new_v4().to_string();
        let (cwd, cancel) = {
            let mut state = cell
                .state
                .lock()
                .map_err(|_| AppError::Invalid("session unavailable".into()))?;
            Self::active(&state, id)?;
            let cancel = antiquota_core::Cancellation::default();
            state.shells.insert(call_id.clone(), cancel.clone());
            (state.meta.cwd.clone(), cancel)
        };
        let _call = ShellCall {
            cell: cell.clone(),
            id: call_id,
        };
        let started = self.clock.now();
        let result = self.shell.run(&cwd, cmd, 600.0, cancel).await?;
        let output = tail(
            &strip_ansi(&format!(
                "{}{}",
                result.output,
                if result.timed_out {
                    "\n(stopped after 600s)"
                } else {
                    ""
                }
            )),
            50_000,
        );
        let event = Event::Shell {
            cmd: cmd.into(),
            output: output.clone(),
            code: result.code,
            secs: ((self.clock.now() - started).max(0.0) * 10.0).round() / 10.0,
        };
        let note = format!(
            "The user ran `{cmd}` in the project folder (exit {}). Output{}:\n```\n{}\n```",
            result
                .code
                .map(|n| n.to_string())
                .unwrap_or_else(|| "timeout".into()),
            if output.chars().count() > 6000 {
                " (last part)"
            } else {
                ""
            },
            tail(&output, 6000)
        );
        let mut state = cell
            .state
            .lock()
            .map_err(|_| AppError::Invalid("session unavailable".into()))?;
        Self::active(&state, id)?;
        self.append(&mut state, event)?;
        let value = serde_json::to_value(
            state
                .records
                .last()
                .ok_or_else(|| AppError::Invalid("shell event was not persisted".into()))?,
        )
        .map_err(|e| AppError::Invalid(e.to_string()))?;
        let mut next = state.meta.clone();
        add_note(&mut next.pending_note, &note);
        self.replace_meta(&mut state, next)?;
        Ok(value)
    }
    pub fn note(&self, id: &str, text: &str) -> Result<Value, AppError> {
        if text.trim().is_empty() {
            return Err(AppError::Invalid("empty note".into()));
        }
        let cell = self.cell(id)?;
        let state = cell
            .state
            .lock()
            .map_err(|_| AppError::Invalid("session unavailable".into()))?;
        Self::active(&state, id)?;
        Ok(self.workspace.note(&state.meta.cwd, text)?)
    }
    pub fn mention_files(&self, id: &str, query: &str) -> Result<Vec<String>, AppError> {
        Ok(self.workspace.mention_files(&self.get(id)?.0.cwd, query)?)
    }
    pub fn browse(&self, path: &str) -> Result<Value, AppError> {
        Ok(self.workspace.browse(path)?)
    }
}
fn add_note(target: &mut String, note: &str) {
    if !target.is_empty() {
        target.push_str("\n\n");
    }
    target.push_str(note);
}
fn strip_ansi(text: &str) -> String {
    let mut chars = text.chars().peekable();
    let mut out = String::new();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for c in chars.by_ref() {
                if ('@'..='~').contains(&c) {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

struct ShellCall {
    cell: Arc<crate::Cell>,
    id: String,
}
impl Drop for ShellCall {
    fn drop(&mut self) {
        if let Ok(mut state) = self.cell.state.lock() {
            state.shells.remove(&self.id);
        }
    }
}
