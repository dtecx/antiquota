//! Owns typed runner context updates and their existing durable metadata events.
use crate::Service;
use antiquota_core::{Event, EventRecord, PortError, StepUsage, StreamEvent, TurnSink};
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
    /// Publishes, but never stores, live counters; the durable turn end adds the final usage once.
    fn usage(&mut self, usage: &StepUsage) -> Result<(), PortError> {
        let cell = self
            .service
            .cell(&self.id)
            .map_err(|e| PortError::Unavailable(e.to_string()))?;
        let state = cell
            .state
            .lock()
            .map_err(|_| PortError::Unavailable("session unavailable".into()))?;
        let mut live = state.meta.clone();
        live.usage.input = live.usage.input.saturating_add(usage.input_tokens);
        live.usage.output = live.usage.output.saturating_add(usage.output_tokens);
        live.usage.thinking = live.usage.thinking.saturating_add(usage.thinking_tokens);
        if usage.context > 0 {
            live.context = usage.context;
        }
        let _ = self.service.events.send(StreamEvent {
            sid: Some(live.id.clone()),
            record: EventRecord::new(
                self.service.clock.now(),
                Event::Meta {
                    meta: Box::new(live),
                },
            ),
        });
        Ok(())
    }
}
