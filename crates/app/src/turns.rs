//! Owns one runner per session, queued messages, cancellation, and turn completion.
use crate::{AppError, Cell, Service};
use antiquota_core::{Cancellation, Event, StepUsage, TurnRequest, TurnResult};
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
    async fn one_turn(self: &Arc<Self>, id: &str, prompt: &str) -> Result<(), AppError> {
        let cell = self.cell(id)?;
        let preferred = cell
            .state
            .lock()
            .map_err(|_| AppError::Invalid("session unavailable".into()))?
            .meta
            .last_slot
            .clone();
        let lease = self
            .accounts
            .acquire(preferred.as_deref())?
            .ok_or_else(|| AppError::Invalid("no available account".into()))?;
        let (request, cancel) = {
            let mut state = cell
                .state
                .lock()
                .map_err(|_| AppError::Invalid("session unavailable".into()))?;
            if state.cancel.is_cancelled() {
                return Ok(());
            }
            let turn = state
                .meta
                .turns
                .checked_add(1)
                .filter(|x| *x <= antiquota_core::MAX_SAFE_INTEGER)
                .ok_or_else(|| AppError::Invalid("turn counter overflow".into()))?;
            let model = state.meta.model.clone();
            let slot = lease.slot().clone();
            self.append(
                &mut state,
                Event::TurnStart {
                    turn,
                    slot: slot.name.clone(),
                    model: model.clone(),
                },
            )?;
            self.snapshot(&mut state)?;
            let request = TurnRequest {
                session: id.into(),
                turn,
                prompt: prompt.into(),
                model,
                cwd: state.meta.cwd.clone(),
                mode: state.meta.mode,
                conversation_id: state.meta.conversation_id.clone(),
                slot,
                state_dir: self.store.state_dir(),
            };
            (request, state.cancel.clone())
        };
        let turn = request.turn;
        let slot = request.slot.name.clone();
        let started = self.clock.now();
        let service = self.clone();
        let sid = id.to_owned();
        let result = self
            .runner
            .run(
                request,
                cancel.clone(),
                Box::new(crate::runner_sink::ApplicationSink { service, id: sid }),
            )
            .await;
        let result = match result {
            Ok(r) => r,
            Err(e) => TurnResult {
                status: if cancel.is_cancelled() {
                    "stopped"
                } else {
                    "ERROR"
                }
                .into(),
                usage: StepUsage::default(),
                error: e.to_string(),
                ..Default::default()
            },
        };
        let mut state = cell
            .state
            .lock()
            .map_err(|_| AppError::Invalid("session unavailable".into()))?;
        if let Some(conv) = result.conversation_id {
            state.meta.conversation_id = Some(conv);
        }
        if !result.error.is_empty() {
            self.append(
                &mut state,
                Event::Notice {
                    kind: "error".into(),
                    text: result.error,
                    data: json!({}),
                },
            )?;
        }
        self.append(
            &mut state,
            Event::TurnEnd {
                turn,
                slot,
                status: result.status,
                usage: result.usage,
                secs: (self.clock.now() - started).max(0.0),
            },
        )?;
        self.snapshot(&mut state)?;
        drop(lease);
        Ok(())
    }
}
