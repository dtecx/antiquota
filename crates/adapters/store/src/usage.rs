//! Owns idempotent projection of durable turn usage into the existing ledger.
use crate::{FileStore, append_json, read_values};
use antiquota_core::{Event, EventRecord, PortError};
use serde_json::json;
impl FileStore {
    pub fn reconcile_usage(&self, sid: &str, records: &[EventRecord]) -> Result<(), PortError> {
        let _writer = self
            .usage_writer
            .lock()
            .map_err(|_| PortError::Unavailable("usage writer failed".into()))?;
        let path = self.root.join("state/usage.jsonl");
        let mut rows = read_values(&path)?;
        let Some(first) = records
            .iter()
            .position(|r| matches!(r.event, Event::Meta { .. }))
        else {
            return Ok(());
        };
        let mut model = String::new();
        let mut turn_models = std::collections::BTreeMap::new();
        for record in &records[first..] {
            match &record.event {
                Event::Meta { meta } => model = meta.model.clone(),
                Event::TurnStart { turn, model, .. } => {
                    turn_models.insert(*turn, model.clone());
                }
                Event::TurnEnd {
                    turn, slot, usage, ..
                } if usage.total_tokens > 0 || usage.input_tokens > 0 => {
                    if rows
                        .iter()
                        .any(|row| row["session"] == sid && row["t"].as_f64() == Some(record.ts))
                    {
                        continue;
                    }
                    let row = json!({"t":record.ts,"slot":slot,"model":turn_models.get(turn).unwrap_or(&model),"session":sid,"input":usage.input_tokens,"output":usage.output_tokens,"thinking":usage.thinking_tokens});
                    append_json(&path, &row)?;
                    rows.push(row);
                }
                _ => {}
            }
        }
        Ok(())
    }
}
