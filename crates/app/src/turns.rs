//! Owns one runner per session, queued messages, cancellation, and turn completion.
use crate::{AppError, Cell, Service};
use antiquota_core::{Cancellation, Event};
use serde_json::{Value, json};
use std::sync::Arc;
impl Service {
    pub fn send(
        self: &Arc<Self>,
        id: &str,
        text: String,
        attachments: Vec<String>,
        kind: String,
    ) -> Result<&'static str, AppError> {
        let cell = self.cell(id)?;
        self.send_cell(id, cell, text, attachments, kind)
    }
    fn error_notice(&self, id: &str, error: &AppError) {
        let _ = self.streamed(
            id,
            Event::Notice {
                kind: "error".into(),
                text: error.to_string(),
                data: json!({}),
            },
        );
    }
    pub(crate) fn send_cell(
        self: &Arc<Self>,
        id: &str,
        cell: Arc<Cell>,
        text: String,
        attachments: Vec<String>,
        kind: String,
    ) -> Result<&'static str, AppError> {
        let mut state = cell
            .state
            .lock()
            .map_err(|_| AppError::Invalid("session unavailable".into()))?;
        Self::active(&state, id)?;
        let item = if attachments.is_empty() && kind == "user" {
            Value::String(text)
        } else {
            json!({"text":text,"attachments":attachments,"kind":kind})
        };
        if state.busy {
            let mut next = state.meta.clone();
            next.queue.push(item);
            self.replace_meta(&mut state, next)?;
            return Ok("queued");
        }
        let prompt = self.admit(&mut state, &item, false)?;
        state.busy = true;
        state.cancel = Cancellation::default();
        drop(state);
        let service = self.clone();
        let id = id.to_owned();
        tokio::spawn(async move {
            service.run_queue(id, prompt, kind).await;
        });
        Ok("started")
    }
    /// Runs turns until nothing follows: each finished turn may continue a goal, retry, or
    /// follow up denied commands; otherwise the next queued message runs.
    async fn run_queue(self: Arc<Self>, id: String, mut prompt: String, mut kind: String) {
        let (mut errors, mut continues) = (0, 0);
        loop {
            let outcome = match self.one_turn(&id, &prompt).await {
                Ok(outcome) => outcome,
                Err(error) => {
                    self.error_notice(&id, &error);
                    Default::default()
                }
            };
            if outcome.ok
                && let Err(error) = self.jobs_from_reply(&id, &outcome.reply)
            {
                self.error_notice(&id, &error);
            }
            if kind == "user" {
                continues = 0;
            }
            let next = match self
                .follow_up(&id, &outcome, &mut errors, &mut continues)
                .await
            {
                Ok(next) => next,
                Err(error) => {
                    self.error_notice(&id, &error);
                    None
                }
            };
            let Ok(cell) = self.cell(&id) else { return };
            let Ok(mut state) = cell.state.lock() else {
                return;
            };
            // Decided under the lock, so a message sent right now is never stranded.
            let item = match next {
                Some((text, next_kind)) if !state.cancel.is_cancelled() => Some((
                    json!({"text": text, "attachments": [], "kind": next_kind}),
                    false,
                )),
                _ if !state.cancel.is_cancelled() && !state.meta.queue.is_empty() => {
                    Some((state.meta.queue[0].clone(), true))
                }
                _ => None,
            };
            if let Some((item, queued)) = item {
                kind = item
                    .get("kind")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("user")
                    .to_owned();
                match self.admit(&mut state, &item, queued) {
                    Ok(value) => prompt = value,
                    Err(error) => {
                        tracing::error!(%error,"queued admission failed");
                        state.busy = false;
                        return;
                    }
                }
                drop(state);
                continue;
            }
            state.busy = false;
            // A background job request may still wait on an approval card.
            state.meta.status = if self.has_pending_approval(&id) {
                "approval"
            } else {
                "idle"
            }
            .into();
            state.meta.status_until = None;
            if let Err(e) = self.snapshot(&mut state) {
                tracing::error!(error=%e,"idle persistence failed");
            }
            return;
        }
    }
}
