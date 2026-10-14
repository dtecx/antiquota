//! Owns account ranking, reset selection and genuine provider-error classification.
use crate::{Account, Failure, Quota, RoutingEntry, Settings};
use std::collections::BTreeMap;
pub const FALLBACK_WALL: f64 = 18_600.0;
pub const COOLDOWNS: [f64; 3] = [60.0, 300.0, 900.0];
pub const AFFINITY_WAIT: f64 = 10.0;
pub const PARKED_SECONDS: f64 = 30.0 * 86_400.0;
/// Routing history entries a read returns when the caller names no count.
pub const ROUTING_READ: usize = 200;
pub fn bucket_of_model(model: &str) -> &'static str {
    if model.starts_with("claude") || model.starts_with("gpt") {
        "cg"
    } else {
        "gemini"
    }
}
pub fn bucket_of_args(args: &[String]) -> &'static str {
    let model = args
        .windows(2)
        .find(|w| w[0] == "--model")
        .map(|w| w[1].as_str())
        .or_else(|| args.iter().find_map(|arg| arg.strip_prefix("--model=")))
        .unwrap_or("");
    bucket_of_model(model)
}
pub fn quota_fresh(settings: &Settings) -> f64 {
    settings
        .quota_poll
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .map(|v| (v * 60.0).max(120.0))
        .unwrap_or(600.0)
}
pub fn headroom(quota: &Quota, bucket: &str) -> (Option<f64>, Option<f64>) {
    let Some(windows) = quota.0.get(bucket) else {
        return (None, None);
    };
    let fraction = windows
        .values()
        .map(|v| v.0)
        .filter(|v| v.is_finite())
        .min_by(f64::total_cmp);
    let reset = windows
        .values()
        .filter_map(|v| v.1)
        .filter(|v| v.is_finite() && *v > 0.0)
        .max_by(f64::total_cmp);
    (fraction, reset)
}
pub fn quota_walls(quota: &Quota, now: f64) -> BTreeMap<String, Option<f64>> {
    quota
        .0
        .iter()
        .map(|(bucket, windows)| {
            let spent: Vec<_> = windows.values().filter(|v| v.0 <= 0.02).collect();
            let reset = spent
                .iter()
                .filter_map(|v| v.1)
                .filter(|v| v.is_finite() && *v > 0.0)
                .max_by(f64::total_cmp)
                .or_else(|| {
                    windows
                        .values()
                        .filter_map(|v| v.1)
                        .filter(|v| v.is_finite() && *v > 0.0)
                        .max_by(f64::total_cmp)
                })
                .unwrap_or(now + FALLBACK_WALL);
            (bucket.clone(), (!spent.is_empty()).then_some(reset + 120.0))
        })
        .collect()
}
pub fn rank_accounts(
    accounts: &[Account],
    bucket: &str,
    preferred: Option<&str>,
    settings: &Settings,
    routes: &[RoutingEntry],
    now: f64,
) -> Vec<String> {
    let reserve = settings
        .reserve
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .unwrap_or(3.0)
        / 100.0;
    let mut used = BTreeMap::new();
    for entry in routes {
        if entry.event == "pick"
            && let Some(slot) = &entry.slot
        {
            used.insert(slot.as_str(), entry.ts);
        }
    }
    let spread = settings.placement == crate::SPREAD;
    let mut live: Vec<_> = accounts
        .iter()
        .filter(|a| !a.paused && a.walls.get(bucket).copied().unwrap_or(0.0) <= now)
        .collect();
    live.sort_by(|a, b| {
        let (ar, at) = headroom(&a.quota, bucket);
        let (br, bt) = headroom(&b.quota, bucket);
        let common = ar
            .is_some_and(|v| v < reserve)
            .cmp(&br.is_some_and(|v| v < reserve))
            .then_with(|| {
                (preferred != Some(a.slot.as_str())).cmp(&(preferred != Some(b.slot.as_str())))
            })
            // Spreading puts accounts without a running call first; filling ignores calls.
            .then_with(|| (spread && a.busy).cmp(&(spread && b.busy)));
        let strategy = match settings.rotation.as_str() {
            "sticky" => a.slot.cmp(&b.slot),
            "round-robin" => used
                .get(a.slot.as_str())
                .copied()
                .unwrap_or(0.0)
                .total_cmp(&used.get(b.slot.as_str()).copied().unwrap_or(0.0)),
            _ => at
                .unwrap_or(f64::INFINITY)
                .total_cmp(&bt.unwrap_or(f64::INFINITY))
                .then_with(|| br.unwrap_or(0.5).total_cmp(&ar.unwrap_or(0.5))),
        };
        common.then(strategy).then_with(|| a.slot.cmp(&b.slot))
    });
    live.into_iter().map(|a| a.slot.clone()).collect()
}
pub fn next_reset(accounts: &[Account], bucket: &str, now: f64) -> f64 {
    accounts
        .iter()
        .filter_map(|a| a.walls.get(bucket))
        .copied()
        .filter(|v| *v > now)
        .min_by(f64::total_cmp)
        .unwrap_or(0.0)
}
pub fn cooldown_strikes(routes: &[RoutingEntry], slot: &str, bucket: &str, now: f64) -> usize {
    routes
        .iter()
        .filter(|r| {
            r.event == "cooldown"
                && r.slot.as_deref() == Some(slot)
                && r.bucket.as_deref() == Some(bucket)
                && now - r.ts < 3600.0
        })
        .count()
}
/// agy's own error channels: stderr of a failed call and error results, never the model's reply.
pub fn error_text(code: i32, out: &str, err: &str) -> String {
    let mut parts = if code != 0 && !err.is_empty() {
        vec![err.to_owned()]
    } else {
        vec![]
    };
    for chunk in std::iter::once(out).chain(out.lines()) {
        let Ok(mut value) = serde_json::from_str::<serde_json::Value>(chunk) else {
            continue;
        };
        if value["result"].is_object() {
            value = value["result"].clone();
        }
        if error_frame(&value) {
            if let Some(object) = value.as_object_mut() {
                object.remove("response");
            }
            parts.push(value.to_string());
        }
    }
    parts.join("\n")
}
/// A JSON frame that agy itself marks as failed.
pub fn error_frame(value: &serde_json::Value) -> bool {
    value.is_object()
        && (value["type"] == "error"
            || value.get("error").is_some_and(|v| !v.is_null() && v != "")
            || value
                .get("status")
                .is_some_and(|v| !v.is_null() && v != "SUCCESS"))
}
pub fn classify_error(text: &str) -> Option<Failure> {
    // The pinned regex release caches literal regexes: https://docs.rs/regex/1.13.1/regex/macro.regex.html
    // Offline first: agy reports a failed token refresh as not signed in when the network is down.
    if regex::regex!(r"(?i)dial tcp|no such host|network is (unreachable|down)|no route to host|temporary failure in name resolution|could not resolve host|getaddrinfo|i/o timeout|TLS handshake timeout").is_match(text) { return Some(Failure::Offline); }
    if regex::regex!(r"(?i)UNAUTHENTICATED|invalid_grant|token (has been )?(expired|revoked)|Authentication required|not (signed|logged) in|re-?authenticate").is_match(text) { return Some(Failure::Auth); }
    if regex::regex!(r"(?i)RESOURCE_EXHAUSTED|quota|\b429\b|rate.?limit|usage limit").is_match(text)
    {
        return Some(Failure::Quota);
    }
    if regex::regex!(r"(?i)UNAVAILABLE|overloaded|capacity|\b50[0234]\b|deadline.?exceeded|internal error|temporar|connection (reset|refused)|timed? ?out|try again").is_match(text) { return Some(Failure::Transient); }
    None
}
#[cfg(test)]
mod tests;
