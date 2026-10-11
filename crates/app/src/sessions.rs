//! Owns session lifecycle commands and compatible metadata updates.
use crate::{AppError, Cell, Service, SessionState};
use antiquota_core::{Cancellation, Event, EventRecord, Mode, Session, SessionPatch};
use std::sync::{Arc, Mutex};
pub fn mode(text: &str) -> Result<Mode, AppError> {
    match text {
        "default" => Ok(Mode::Default),
        "accept-edits" => Ok(Mode::AcceptEdits),
        "plan" => Ok(Mode::Plan),
        "bypass" => Ok(Mode::Bypass),
        _ => Err(AppError::Invalid(format!("unknown mode: {text}"))),
    }
}
impl Service {
    pub fn create(
        &self,
        cwd: &str,
        title: &str,
        model: &str,
        mode: Mode,
    ) -> Result<Session, AppError> {
        self.create_in(cwd, title, model, mode, false)
    }
    /// Creates a session; with `worktree` it gets its own branch and folder so parallel
    /// sessions never edit the same files.
    pub fn create_in(
        &self,
        cwd: &str,
        title: &str,
        model: &str,
        mode: Mode,
        worktree: bool,
    ) -> Result<Session, AppError> {
        let mut cwd = self.store.validate_cwd(cwd)?;
        let reservation = loop {
            let id = uuid::Uuid::new_v4().simple().to_string()[..12].to_owned();
            if self.store.exists(&id)? {
                continue;
            }
            let mut reserved = self
                .reserved
                .lock()
                .map_err(|_| AppError::Invalid("session reservation unavailable".into()))?;
            let exists = self
                .cells
                .read()
                .map_err(|_| AppError::Invalid("session registry unavailable".into()))?
                .contains_key(&id);
            if !exists && reserved.insert(id.clone()) {
                break Reservation { service: self, id };
            }
        };
        let id = reservation.id.clone();
        let mut record = None;
        if worktree {
            let (made, work) = self.new_worktree(&cwd, &id, title)?;
            record = Some(made.to_value());
            cwd = work;
        }
        let now = self.clock.now();
        if !now.is_finite() || now < 0.0 {
            return Err(AppError::Invalid("invalid clock instant".into()));
        }
        let meta = Session {
            id: id.clone(),
            title: title.into(),
            cwd,
            model: model.into(),
            mode,
            created: now,
            updated: now,
            worktree: record,
            ..Default::default()
        };
        let record = EventRecord::new(
            now,
            Event::Meta {
                meta: Box::new(meta.clone()),
            },
        );
        self.store.append(&id, &record)?;
        self.store.save(&meta)?;
        let state = SessionState {
            meta: meta.clone(),
            records: vec![record.clone()],
            busy: false,
            cancel: Cancellation::default(),
            shells: std::collections::BTreeMap::new(),
            deleting: false,
        };
        self.cells
            .write()
            .map_err(|_| AppError::Invalid("session registry unavailable".into()))?
            .insert(
                id.clone(),
                Arc::new(Cell {
                    state: Mutex::new(state),
                }),
            );
        let _ = self.events.send(antiquota_core::StreamEvent {
            sid: Some(id),
            record,
        });
        let result = meta;
        Ok(result)
    }
    pub fn get(&self, id: &str) -> Result<(Session, Vec<EventRecord>), AppError> {
        let cell = self.cell(id)?;
        let state = cell
            .state
            .lock()
            .map_err(|_| AppError::Invalid("session unavailable".into()))?;
        Self::active(&state, id)?;
        Ok((
            state.meta.clone(),
            state
                .records
                .iter()
                .filter(|r| r.event.is_transcript())
                .cloned()
                .map(|mut record| {
                    if let Event::Notice { kind, data, .. } = &mut record.event
                        && kind == "denied"
                    {
                        crate::notices::denial_suggestions(data);
                    }
                    record
                })
                .collect(),
        ))
    }
    pub fn patch(&self, id: &str, fields: &SessionPatch) -> Result<Session, AppError> {
        let cell = self.cell(id)?;
        let mut state = cell
            .state
            .lock()
            .map_err(|_| AppError::Invalid("session unavailable".into()))?;
        Self::active(&state, id)?;
        let mut next = state.meta.clone();
        if let Some(title) = &fields.title {
            next.title = title.clone();
        }
        if let Some(model) = &fields.model {
            next.model = model.clone();
        }
        if let Some(effort) = &fields.effort {
            next.effort = effort.clone();
        }
        if let Some(mode) = fields.mode {
            next.mode = mode;
        }
        if let Some(pinned) = fields.pinned {
            next.pinned = pinned;
        }
        if let Some(archived) = fields.archived {
            next.archived = archived;
        }
        self.replace_meta(&mut state, next)?;
        Ok(state.meta.clone())
    }
    pub(crate) fn active(state: &SessionState, id: &str) -> Result<(), AppError> {
        if state.deleting {
            Err(AppError::Missing(id.into()))
        } else {
            Ok(())
        }
    }
    pub fn stop(&self, id: &str) -> Result<(), AppError> {
        let cell = self.cell(id)?;
        let mut state = cell
            .state
            .lock()
            .map_err(|_| AppError::Invalid("session unavailable".into()))?;
        Self::active(&state, id)?;
        self.stop_state(&mut state)
    }
    fn stop_state(&self, state: &mut SessionState) -> Result<(), AppError> {
        state.cancel.cancel();
        for cancel in state.shells.values() {
            cancel.cancel();
        }
        // Stop must take effect in memory even when persistence fails.
        state.meta.queue.clear();
        if let Some(goal) = &mut state.meta.goal {
            goal.state = "paused".into();
        }
        self.deny_pending(state)?;
        self.snapshot(state)
    }
    pub async fn delete(&self, id: &str) -> Result<(), AppError> {
        let cell = self.cell(id)?;
        {
            let mut state = cell
                .state
                .lock()
                .map_err(|_| AppError::Invalid("session unavailable".into()))?;
            Self::active(&state, id)?;
            state.deleting = true;
            self.stop_state(&mut state)?;
        }
        self.wait_idle(id).await;
        self.term_kill(id);
        let state = cell
            .state
            .lock()
            .map_err(|_| AppError::Invalid("session unavailable".into()))?;
        self.store.project_usage(id, &state.records)?;
        self.store.delete(id)?;
        drop(state);
        self.cells
            .write()
            .map_err(|_| AppError::Invalid("session registry unavailable".into()))?
            .remove(id);
        let _ = self.events.send(antiquota_core::StreamEvent {
            sid: Some(id.into()),
            record: EventRecord::new(self.clock.now(), Event::Deleted {}),
        });
        Ok(())
    }
}

struct Reservation<'a> {
    service: &'a Service,
    id: String,
}
impl Drop for Reservation<'_> {
    fn drop(&mut self) {
        if let Ok(mut reserved) = self.service.reserved.lock() {
            reserved.remove(&self.id);
        }
    }
}
