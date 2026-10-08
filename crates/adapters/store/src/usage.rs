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

impl FileStore {
    pub(crate) fn history(&self, days: u32, now: f64) -> Result<serde_json::Value, PortError> {
        use chrono::{DateTime, Duration, Local};
        use serde_json::{Value, json};
        use std::collections::BTreeMap;
        let today = DateTime::from_timestamp(now as i64, 0)
            .ok_or_else(|| PortError::Invalid("invalid usage date".into()))?
            .with_timezone(&Local)
            .date_naive();
        let days = days.clamp(1, 90);
        let first = today - Duration::days(i64::from(days) - 1);
        let mut rows: BTreeMap<String, Value> = (0..days)
            .map(|i| {
                (
                    (first + Duration::days(i64::from(i))).to_string(),
                    json!({"total":0,"turns":0,"by":{}}),
                )
            })
            .collect();
        let mut accounts = json!({});
        let mut models = json!({});
        let add = |value: &mut Value, n: u64| -> Result<(), PortError> {
            let total = value
                .as_u64()
                .unwrap_or(0)
                .checked_add(n)
                .filter(|n| *n <= antiquota_core::MAX_SAFE_INTEGER)
                .ok_or_else(|| PortError::Invalid("usage counter overflow".into()))?;
            *value = json!(total);
            Ok(())
        };
        for row in read_values(&self.root.join("state/usage.jsonl"))? {
            let Some(t) = row["t"].as_f64().filter(|t| t.is_finite()) else {
                continue;
            };
            let Some(day) =
                DateTime::from_timestamp(t as i64, 0).map(|t| t.with_timezone(&Local).date_naive())
            else {
                continue;
            };
            if day < first || day > today {
                continue;
            }
            let Some(slot) = row["slot"].as_str() else {
                continue;
            };
            let values =
                ["input", "output", "thinking"].map(|k| row.get(k).map_or(Some(0), Value::as_u64));
            let [Some(input), Some(output), Some(thinking)] = values else {
                continue;
            };
            let n = input
                .checked_add(output)
                .and_then(|n| n.checked_add(thinking))
                .ok_or_else(|| PortError::Invalid("usage counter overflow".into()))?;
            let r = rows.get_mut(&day.to_string()).unwrap();
            add(&mut r["total"], n)?;
            add(&mut r["turns"], 1)?;
            add(&mut r["by"][slot], n)?;
            let model = row["model"]
                .as_str()
                .filter(|s| !s.is_empty())
                .unwrap_or("?");
            add(&mut models[model], n)?;
            if accounts.get(slot).is_none() {
                accounts[slot] = json!({"tokens":0,"turns":0,"today":0,"input":0,"output":0});
            }
            for (key, n) in [
                ("tokens", n),
                ("turns", 1),
                ("input", input),
                ("output", output),
                ("today", if day == today { n } else { 0 }),
            ] {
                add(&mut accounts[slot][key], n)?;
            }
        }
        let days: Vec<_> = rows
            .into_iter()
            .map(|(date, mut r)| {
                r["date"] = json!(date);
                r
            })
            .collect();
        Ok(json!({"days":days,"accounts":accounts,"models":models}))
    }
}
