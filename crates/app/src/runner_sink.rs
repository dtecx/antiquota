//! Owns typed runner context updates and their existing durable metadata events.
use crate::Service;
use antiquota_core::{Event, PortError, TurnSink};
use std::sync::Arc;
pub(crate) struct ApplicationSink {
    pub service: Arc<Service>,
    pub id: String,
}
impl TurnSink for ApplicationSink {
    fn event(&mut self, event: Event) -> Result<(), PortError> {
        self.service.streamed(&self.id, event)
    }
    fn conversation_id(&mut self, conversation: &str) -> Result<(), PortError> {
        let cell = self
            .service
            .cell(&self.id)
            .map_err(|e| PortError::Unavailable(e.to_string()))?;
        let mut state = cell
            .state
            .lock()
            .map_err(|_| PortError::Unavailable("session unavailable".into()))?;
        let mut next = state.meta.clone();
        next.conversation_id = Some(conversation.into());
        self.service
            .replace_meta(&mut state, next)
            .map_err(|e| PortError::Unavailable(e.to_string()))
    }
}
