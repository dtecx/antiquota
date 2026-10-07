//! Owns the bounded account-handoff decision after a provider attempt.
use crate::Feature;
use antiquota_core::{Command, PortError, Session, Turn};
#[derive(Debug, Default)]
pub struct Handoff;
impl Feature for Handoff {
    fn after_turn(&self, session: &Session, turn: &Turn) -> Result<Vec<Command>, PortError> {
        let (delay, fresh) = match turn.status.as_str() {
            "quota" | "auth" => (0.0, false),
            "transient" => (
                [10.0, 30.0]
                    .get(turn.retries as usize)
                    .copied()
                    .unwrap_or(0.0),
                false,
            ),
            "conversation_lost" => (0.0, true),
            _ => return Ok(vec![]),
        };
        Ok(vec![Command::ContinueTurn {
            session: session.id.clone(),
            delay,
            fresh,
        }])
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_retryable_provider_outcomes_continue() {
        let session = Session::default();
        for status in ["SUCCESS", "stopped", "ERROR"] {
            assert!(
                Handoff
                    .after_turn(
                        &session,
                        &Turn {
                            status: status.into(),
                            ..Default::default()
                        }
                    )
                    .unwrap()
                    .is_empty()
            );
        }
        for (status, retries, delay, fresh) in [
            ("quota", 0, 0.0, false),
            ("auth", 0, 0.0, false),
            ("transient", 0, 10.0, false),
            ("transient", 1, 30.0, false),
            ("transient", 2, 0.0, false),
            ("conversation_lost", 0, 0.0, true),
        ] {
            assert!(
                matches!(&Handoff.after_turn(&session,&Turn{status:status.into(),retries,..Default::default()}).unwrap()[0],Command::ContinueTurn{delay:d,fresh:f,..} if *d==delay && *f==fresh)
            );
        }
    }
}
