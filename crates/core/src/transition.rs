//! Owns deterministic session reduction and snapshot-boundary replay.
use crate::{Event, EventRecord, MAX_SAFE_INTEGER, Session};
use thiserror::Error;
#[derive(Debug, Error)]
pub enum DomainError {
    #[error("token counter exceeds the safe integer range")]
    CounterOverflow,
}
fn add(left: u64, right: u64) -> Result<u64, DomainError> {
    left.checked_add(right)
        .filter(|x| *x <= MAX_SAFE_INTEGER)
        .ok_or(DomainError::CounterOverflow)
}
pub fn reduce(session: &mut Session, record: &EventRecord) -> Result<(), DomainError> {
    match &record.event {
        Event::Meta { meta } => *session = *meta.clone(),
        Event::User {
            text,
            kind,
            steered,
            ..
        } => {
            if session.title.is_empty() && kind == "user" {
                session.title = text.lines().next().unwrap_or("").chars().take(60).collect();
            }
            if *steered != Some(true)
                && session.status != "idle"
                && session.queue.first().is_some_and(|item| {
                    item.as_str()
                        .or_else(|| item.get("text").and_then(serde_json::Value::as_str))
                        == Some(text)
                        && item
                            .get("kind")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("user")
                            == kind
                })
            {
                session.queue.remove(0);
            }
        }
        Event::TurnStart { turn, slot, .. } => {
            session.turns = *turn;
            session.last_slot = Some(slot.clone());
            session.status = "running".into();
        }
        Event::TurnEnd { usage, .. } => {
            let input = add(session.usage.input, usage.input_tokens)?;
            let output = add(session.usage.output, usage.output_tokens)?;
            let thinking = add(session.usage.thinking, usage.thinking_tokens)?;
            let cache = add(session.usage.cache, usage.cache_read_tokens)?;
            session.usage.input = input;
            session.usage.output = output;
            session.usage.thinking = thinking;
            session.usage.cache = cache;
            if usage.context > 0 {
                session.context = usage.context;
            }
        }
        _ => {}
    }
    Ok(())
}
pub fn replay(mut legacy: Session, records: &[EventRecord]) -> Result<Session, DomainError> {
    if let Some(boundary) = records
        .iter()
        .rposition(|r| matches!(r.event, Event::Meta { .. }))
    {
        for record in &records[boundary..] {
            reduce(&mut legacy, record)?;
        }
    }
    Ok(legacy)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::StepUsage;
    use proptest::prelude::*;
    #[test]
    fn legacy_snapshot_is_not_counted_again() {
        let s = Session {
            usage: crate::Usage {
                input: 100,
                ..Default::default()
            },
            ..Default::default()
        };
        let r = EventRecord::new(
            1.0,
            Event::TurnEnd {
                turn: 1,
                slot: "a".into(),
                status: "SUCCESS".into(),
                usage: StepUsage {
                    input_tokens: 100,
                    ..Default::default()
                },
                secs: 0.0,
            },
        );
        assert_eq!(replay(s, &[r]).unwrap().usage.input, 100);
    }
    proptest! { #[test] fn latest_snapshot_replays_only_tail(a in 0u64..10000, b in 0u64..10000) {
        let base=Session { usage: crate::Usage { input:a, ..Default::default() }, ..Default::default() };
        let records=[EventRecord::new(1.0,Event::Meta { meta:Box::new(base) }),EventRecord::new(2.0,Event::TurnEnd { turn:1,slot:"a".into(),status:"SUCCESS".into(),usage:StepUsage { input_tokens:b,..Default::default() },secs:0.0 })];
        prop_assert_eq!(replay(Session::default(),&records).unwrap().usage.input,a+b);
    } }
}
