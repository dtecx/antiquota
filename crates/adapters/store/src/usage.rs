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
                    let mut row = json!({"t":record.ts,"slot":slot,"model":turn_models.get(turn).unwrap_or(&model),"session":sid,"input":usage.input_tokens,"output":usage.output_tokens,"thinking":usage.thinking_tokens});
                    if usage.cache_read_tokens > 0 {
                        row["cache"] = json!(usage.cache_read_tokens);
                    }
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
    pub(crate) fn history(
        &self,
        days: u32,
        now: f64,
        detail: bool,
    ) -> Result<serde_json::Value, PortError> {
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
        // The detail splits every total into its parts, including the cached input agy reused.
        let zero = json!({"input":0,"output":0,"thinking":0,"cache":0,"turns":0});
        let mut parts = json!({"period":zero,"today":zero,"accounts":{},"models":{}});
        let mut part_days: BTreeMap<String, Value> = rows
            .keys()
            .map(|date| (date.clone(), zero.clone()))
            .collect();
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
            // Older rows have no cache count; an unreadable one counts as none.
            let cache = row["cache"].as_u64().unwrap_or(0);
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
            if detail {
                let counts = [
                    ("input", input),
                    ("output", output),
                    ("thinking", thinking),
                    ("cache", cache),
                    ("turns", 1),
                ];
                let tally = |target: &mut Value| -> Result<(), PortError> {
                    for (key, n) in counts {
                        add(&mut target[key], n)?;
                    }
                    Ok(())
                };
                tally(&mut parts["period"])?;
                if day == today {
                    tally(&mut parts["today"])?;
                }
                tally(part_days.get_mut(&day.to_string()).unwrap())?;
                for (group, key) in [("accounts", slot), ("models", model)] {
                    if parts[group].get(key).is_none() {
                        parts[group][key] = zero.clone();
                    }
                    tally(&mut parts[group][key])?;
                }
            }
        }
        let days: Vec<_> = rows
            .into_iter()
            .map(|(date, mut r)| {
                r["date"] = json!(date);
                r
            })
            .collect();
        let mut result = json!({"days":days,"accounts":accounts,"models":models});
        if detail {
            parts["days"] = json!(part_days.into_values().collect::<Vec<_>>());
            result["detail"] = parts;
        }
        Ok(result)
    }
}
