//! Owns pure feature hooks; implemented feature modules join this trait later.
pub mod handoff;
use antiquota_core::{Command, Event, PortError, Session, Turn};
pub trait Feature: Send + Sync {
    fn before_turn(&self, _session: &Session, _turn: &Turn) -> Result<Vec<Command>, PortError> {
        Ok(vec![])
    }
    fn on_event(&self, _session: &Session, _event: &Event) -> Result<Vec<Command>, PortError> {
        Ok(vec![])
    }
    fn after_turn(&self, _session: &Session, _turn: &Turn) -> Result<Vec<Command>, PortError> {
        Ok(vec![])
    }
}
