//! Owns durable append, snapshot boundaries, and ordered event publication.
use crate::{AppError, Service, SessionState};
use antiquota_core::{Event, EventRecord, PortError, StreamEvent, reduce};
impl Service {
    pub(crate) fn append(&self, state: &mut SessionState, event: Event) -> Result<(), AppError> {
        let timestamp = if matches!(event, Event::TurnEnd { .. }) {
            self.store
                .turn_timestamp(&state.meta.id, self.clock.now(), &state.records)?
        } else {
            let mut stamp = self.clock.now();
            if !stamp.is_finite() || stamp < 0.0 {
                return Err(AppError::Invalid("invalid clock instant".into()));
            }
            if event.is_durable() {
                while state.records.iter().any(|record| record.ts == stamp) {
                    stamp = stamp.next_up();
                    if !stamp.is_finite() {
                        return Err(AppError::Invalid(
                            "no finite event timestamp remains".into(),
                        ));
                    }
                }
            }
            stamp
        };
        if let Event::Tool {
            turn,
            name,
            params,
            state: phase,
            ..
        } = &event
            && [
                "write_to_file",
                "replace_file_content",
                "multi_replace_file_content",
                "sed_file",
            ]
            .contains(&name.as_str())
            && let Some(path) = params["TargetFile"]
                .as_str()
                .or_else(|| params["File"].as_str())
        {
            self.store.checkpoint(
                &state.meta.id,
                *turn,
                &state.meta.cwd,
                path,
                phase == "running",
            )?;
        }
        let record = EventRecord::new(timestamp, event);
        if record.event.is_durable() {
            let mut next = state.meta.clone();
            reduce(&mut next, &record).map_err(|e| AppError::Invalid(e.to_string()))?;
            self.store.append(&state.meta.id, &record)?;
            state.meta = next;
            state.records.push(record.clone());
            if matches!(record.event, Event::TurnEnd { .. }) {
                self.store.project_usage(&state.meta.id, &state.records)?;
            }
            self.store.save(&state.meta)?;
            if matches!(record.event, Event::TurnEnd { .. }) {
                self.store.save_state_document(
                    &state.meta.id,
                    &antiquota_core::state_document::document(&state.meta, &state.records),
                )?;
            }
        }
        let _ = self.events.send(StreamEvent {
            sid: Some(state.meta.id.clone()),
            record,
        });
        Ok(())
    }
    pub(crate) fn snapshot(&self, state: &mut SessionState) -> Result<(), AppError> {
        self.replace_meta(state, state.meta.clone())
    }
    pub(crate) fn replace_meta(
        &self,
        state: &mut SessionState,
        mut meta: antiquota_core::Session,
    ) -> Result<(), AppError> {
        meta.updated = self.clock.now();
        self.append(
            state,
            Event::Meta {
                meta: Box::new(meta),
            },
        )
    }
    pub(crate) fn streamed(&self, id: &str, event: Event) -> Result<(), PortError> {
        let cell = self
            .cell(id)
            .map_err(|e| PortError::Unavailable(e.to_string()))?;
        let mut state = cell
            .state
            .lock()
            .map_err(|_| PortError::Unavailable("session unavailable".into()))?;
        self.append(&mut state, event)
            .map_err(|e| PortError::Unavailable(e.to_string()))
    }
}
