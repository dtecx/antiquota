//! Owns durable user-input admission before any runner or account operation.
use crate::{AppError, Service, SessionState};
use antiquota_core::Event;
use serde_json::Value;
impl Service {
    pub(crate) fn admit(
        &self,
        state: &mut SessionState,
        item: &Value,
        queued: bool,
    ) -> Result<String, AppError> {
        let text = item
            .as_str()
            .or_else(|| item["text"].as_str())
            .unwrap_or("")
            .to_owned();
        let kind = item
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("user")
            .to_owned();
        let paths: Vec<String> = item
            .get("attachments")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
        let attachments = self.store.attachments(&state.meta.id, &paths)?;
        let mut next = state.meta.clone();
        next.status = "waiting".into();
        next.status_until = None;
        if queued {
            next.queue.remove(0);
        }
        if next.title.is_empty() && kind == "user" {
            next.title = text.lines().next().unwrap_or("").chars().take(60).collect();
        }
        let note = std::mem::take(&mut next.pending_note);
        self.append(
            state,
            Event::User {
                text: text.clone(),
                kind,
                attachments: Some(attachments.clone()),
                steered: None,
            },
        )?;
        self.replace_meta(state, next)?;
        self.name_branch(state)?;
        let mut prompt = self.expand_mentions(&text, &state.meta.cwd);
        for attachment in attachments {
            prompt.push_str(&format!("\nAttached file: {}", attachment.path));
        }
        if !note.is_empty() {
            // A rewind, compact or worktree change happened since the last turn.
            prompt = format!("{note}\n\n{prompt}");
        }
        if state.meta.conversation_id.is_none() {
            // The request itself stays last.
            prompt = format!("{}\n\n{prompt}", antiquota_core::jobs::JOBS_HINT);
        }
        Ok(prompt)
    }
}
