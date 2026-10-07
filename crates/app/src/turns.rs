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
            service.run_queue(id, prompt).await;
        });
        Ok("started")
    }
    async fn run_queue(self: Arc<Self>, id: String, mut prompt: String) {
        loop {
            if let Err(error) = self.one_turn(&id, &prompt).await {
                let _ = self.streamed(
                    &id,
                    Event::Notice {
                        kind: "error".into(),
                        text: error.to_string(),
                        data: json!({}),
                    },
                );
            }
            let Ok(cell) = self.cell(&id) else { return };
            let Ok(mut state) = cell.state.lock() else {
                return;
            };
            if !state.cancel.is_cancelled() && !state.meta.queue.is_empty() {
                let item = state.meta.queue[0].clone();
                match self.admit(&mut state, &item, true) {
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
            state.meta.status = "idle".into();
            state.meta.status_until = None;
            if let Err(e) = self.snapshot(&mut state) {
                tracing::error!(error=%e,"idle persistence failed");
            }
            return;
        }
    }
}
