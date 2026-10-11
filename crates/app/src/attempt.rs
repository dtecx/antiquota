//! Owns one leased provider attempt and its durable turn boundaries.
use crate::{AppError, Service};
use antiquota_core::{AccountLease, Cancellation, Event, StepUsage, TurnRequest, TurnResult};
use serde_json::json;
use std::sync::Arc;
impl Service {
    pub(crate) async fn attempt(
        self: &Arc<Self>,
        id: &str,
        prompt: &str,
        lease: &dyn AccountLease,
        cancel: &Cancellation,
    ) -> Result<TurnResult, AppError> {
        self.announce_accounts()?;
        self.accounts.prepare(lease.slot())?;
        let cell = self.cell(id)?;
        let request = {
            let mut state = cell
                .state
                .lock()
                .map_err(|_| AppError::Invalid("session unavailable".into()))?;
            let turn = state
                .meta
                .turns
                .checked_add(1)
                .filter(|v| *v <= antiquota_core::MAX_SAFE_INTEGER)
                .ok_or_else(|| AppError::Invalid("turn counter overflow".into()))?;
            let model = state.meta.model.clone();
            let conversation_id = state.meta.conversation_id.clone();
            self.append(
                &mut state,
                Event::TurnStart {
                    turn,
                    slot: lease.slot().name.clone(),
                    model: model.clone(),
                },
            )?;
            self.snapshot(&mut state)?;
            TurnRequest {
                session: id.into(),
                turn,
                prompt: prompt.into(),
                model,
                cwd: state.meta.cwd.clone(),
                mode: state.meta.mode,
                conversation_id,
                slot: lease.slot().clone(),
                state_dir: self.store.state_dir(),
                add_dirs: self.store.readable_dirs(id),
            }
        };
        let turn = request.turn;
        let resumed = request.conversation_id.is_some();
        let started = self.clock.now();
        let result = self
            .runner
            .run(
                request,
                cancel.clone(),
                Box::new(crate::runner_sink::ApplicationSink {
                    service: self.clone(),
                    id: id.into(),
                }),
            )
            .await;
        let mut result = match result {
            Ok(result) => result,
            Err(error) => TurnResult {
                status: if cancel.is_cancelled() {
                    "stopped"
                } else {
                    "ERROR"
                }
                .into(),
                error: error.to_string(),
                usage: StepUsage::default(),
                ..Default::default()
            },
        };
        if resumed
            && result.status != "SUCCESS"
            && result.status != "stopped"
            && !["quota", "auth", "transient"].contains(&result.status.as_str())
            && result.error.to_ascii_lowercase().contains("conversation")
        {
            result.status = "conversation_lost".into();
        }
        let mut state = cell
            .state
            .lock()
            .map_err(|_| AppError::Invalid("session unavailable".into()))?;
        if let Some(conversation) = &result.conversation_id {
            state.meta.conversation_id = Some(conversation.clone());
        }
        if !result.error.is_empty()
            && !["quota", "auth", "transient", "conversation_lost"]
                .contains(&result.status.as_str())
        {
            self.append(
                &mut state,
                Event::Notice {
                    kind: "error".into(),
                    text: result.error.clone(),
                    data: json!({}),
                },
            )?;
        }
        self.append(
            &mut state,
            Event::TurnEnd {
                turn,
                slot: lease.slot().name.clone(),
                status: result.status.clone(),
                usage: result.usage.clone(),
                secs: (self.clock.now() - started).max(0.0),
            },
        )?;
        self.snapshot(&mut state)?;
        Ok(result)
    }
}
