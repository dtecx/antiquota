//! Owns the account API projection and local-day usage totals.
use crate::{FileAccounts, files};
use antiquota_core::{Account, MAX_SAFE_INTEGER, PortError, Quota, TodayUsage};
use chrono::{Local, TimeZone, Timelike};
use std::collections::BTreeMap;
pub fn accounts(accounts: &FileAccounts, now: f64) -> Result<Vec<Account>, PortError> {
    // A status read briefly takes a lease; concurrent readers must not look like workers.
    let _read = accounts
        .changes
        .lock()
        .map_err(|_| PortError::Unavailable("account status unavailable".into()))?;
    let today = usage_today(accounts, now)?;
    let mut rows = vec![];
    for slot in accounts.slots()? {
        let busy = accounts.lease(slot.clone())?.is_none();
        let snapshot = files::read_json(
            &accounts
                .root
                .join("state/quota")
                .join(format!("{}.json", slot.name)),
        )
        .unwrap_or_default();
        let quota = cached_quota(&snapshot["quota"]);
        let verify = files::read_json(
            &accounts
                .root
                .join("state/verify")
                .join(format!("{}.json", slot.name)),
        )
        .and_then(|v| v["url"].as_str().map(str::to_owned));
        let (mut walls, mut why) = (BTreeMap::new(), BTreeMap::new());
        for bucket in ["gemini", "cg"] {
            let (until, reason) = files::wall(accounts, &slot.name, bucket, now);
            walls.insert(bucket.into(), until);
            why.insert(bucket.into(), reason);
        }
        rows.push(Account {
            slot: slot.name.clone(),
            busy,
            probed: snapshot["at"].as_f64(),
            quota,
            walls,
            why,
            paused: accounts.root.join("state/paused").join(&slot.name).exists(),
            verify,
            today: today.get(&slot.name).cloned().unwrap_or_default(),
        });
    }
    Ok(rows)
}
pub fn usage_today(
    accounts: &FileAccounts,
    now: f64,
) -> Result<BTreeMap<String, TodayUsage>, PortError> {
    // Chrono's safe Local API keeps the existing operating-system calendar semantics.
    // https://docs.rs/chrono/0.4.45/chrono/struct.Local.html
    let midnight = Local
        .timestamp_opt(now.floor() as i64, 0)
        .single()
        .and_then(|v| v.with_hour(0))
        .and_then(|v| v.with_minute(0))
        .and_then(|v| v.with_second(0))
        .map(|v| v.timestamp() as f64)
        .unwrap_or(now);
    let mut totals: BTreeMap<String, TodayUsage> = BTreeMap::new();
    for row in antiquota_store::read_values(&accounts.root.join("state/usage.jsonl"))? {
        let Some(stamp) = row["t"].as_f64().filter(|v| v.is_finite()) else {
            continue;
        };
        if stamp < midnight {
            continue;
        }
        let values = ["input", "output", "thinking"].map(|key| match row.get(key) {
            None => Some(0),
            Some(value) => value.as_u64(),
        });
        let [Some(input), Some(output), Some(thinking)] = values else {
            continue;
        };
        let Some(slot) = row["slot"].as_str() else {
            continue;
        };
        let entry = totals.entry(slot.into()).or_insert_with(|| TodayUsage {
            input: Some(0),
            output: Some(0),
            thinking: Some(0),
            turns: Some(0),
            cache: None,
        });
        // Older rows have no cache count; an unreadable one counts as none.
        if let Some(cache) = row["cache"].as_u64().filter(|n| *n > 0) {
            entry.cache = Some(
                entry
                    .cache
                    .unwrap_or(0)
                    .checked_add(cache)
                    .filter(|v| *v <= MAX_SAFE_INTEGER)
                    .ok_or_else(|| {
                        PortError::Invalid("account usage exceeds safe integer range".into())
                    })?,
            );
        }
        for (target, key) in [
            (&mut entry.input, input),
            (&mut entry.output, output),
            (&mut entry.thinking, thinking),
        ] {
            *target = Some(
                target
                    .unwrap_or(0)
                    .checked_add(key)
                    .filter(|v| *v <= MAX_SAFE_INTEGER)
                    .ok_or_else(|| {
                        PortError::Invalid("account usage exceeds safe integer range".into())
                    })?,
            );
        }
        entry.turns = Some(
            entry
                .turns
                .unwrap_or(0)
                .checked_add(1)
                .filter(|v| *v <= MAX_SAFE_INTEGER)
                .ok_or_else(|| {
                    PortError::Invalid("account turns exceed safe integer range".into())
                })?,
        );
    }
    Ok(totals)
}

fn cached_quota(value: &serde_json::Value) -> Quota {
    let pools = value
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(bucket, windows)| {
            let windows = windows
                .as_object()?
                .iter()
                .filter_map(|(window, value)| {
                    let parts = value.as_array()?;
                    if parts.len() != 2 {
                        return None;
                    }
                    let fraction = parts[0].as_f64().filter(|v| v.is_finite())?;
                    let reset = if parts[1].is_null() {
                        None
                    } else {
                        Some(parts[1].as_f64().filter(|v| v.is_finite())?)
                    };
                    Some((window.clone(), (fraction, reset)))
                })
                .collect::<BTreeMap<_, _>>();
            Some((bucket.clone(), windows))
        })
        .collect();
    Quota(pools)
}
