//! Owns event decoding without Serde buffering that loses opaque numeric precision.
use crate::{Approval, Event, EventRecord, StreamEvent};
use serde::{
    Deserialize, Deserializer,
    de::{DeserializeOwned, Error},
};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
fn field<T: DeserializeOwned>(
    map: &Map<String, Value>,
    name: &str,
) -> Result<T, serde_json::Error> {
    serde_json::from_str(&map.get(name).unwrap_or(&Value::Null).to_string())
}
fn optional<T: DeserializeOwned + Default>(
    map: &Map<String, Value>,
    name: &str,
) -> Result<T, serde_json::Error> {
    if map.contains_key(name) {
        field(map, name)
    } else {
        Ok(T::default())
    }
}
fn decode(map: &Map<String, Value>) -> Result<(Event, &'static [&'static str]), serde_json::Error> {
    let kind: String = field(map, "t")?;
    let result = match kind.as_str() {
        "meta" => (
            Event::Meta {
                meta: field(map, "meta")?,
            },
            &["t", "meta"][..],
        ),
        "user" => (
            Event::User {
                text: field(map, "text")?,
                kind: map
                    .get("kind")
                    .and_then(Value::as_str)
                    .unwrap_or("user")
                    .into(),
                attachments: optional(map, "attachments")?,
                steered: optional(map, "steered")?,
            },
            &["t", "text", "kind", "attachments", "steered"][..],
        ),
        "turn_start" => (
            Event::TurnStart {
                turn: field(map, "turn")?,
                slot: field(map, "slot")?,
                model: field(map, "model")?,
            },
            &["t", "turn", "slot", "model"][..],
        ),
        "delta" => (
            Event::Delta {
                turn: field(map, "turn")?,
                step: field(map, "step")?,
                text: field(map, "text")?,
            },
            &["t", "turn", "step", "text"][..],
        ),
        "assistant" => (
            Event::Assistant {
                turn: field(map, "turn")?,
                step: field(map, "step")?,
                text: field(map, "text")?,
            },
            &["t", "turn", "step", "text"][..],
        ),
        "thinking" => (
            Event::Thinking {
                turn: field(map, "turn")?,
                step: field(map, "step")?,
                text: field(map, "text")?,
                tokens: optional(map, "tokens")?,
            },
            &["t", "turn", "step", "text", "tokens"][..],
        ),
        "tool" => (
            Event::Tool {
                turn: field(map, "turn")?,
                step: field(map, "step")?,
                name: field(map, "name")?,
                params: optional(map, "params")?,
                state: field(map, "state")?,
                secs: optional(map, "secs")?,
                output: optional(map, "output")?,
                diff: optional(map, "diff")?,
                error: optional(map, "error")?,
            },
            &[
                "t", "turn", "step", "name", "params", "state", "secs", "output", "diff", "error",
            ][..],
        ),
        "turn_end" => (
            Event::TurnEnd {
                turn: field(map, "turn")?,
                slot: field(map, "slot")?,
                status: field(map, "status")?,
                usage: field(map, "usage")?,
                secs: field(map, "secs")?,
            },
            &["t", "turn", "slot", "status", "usage", "secs"][..],
        ),
        "notice" => (
            Event::Notice {
                kind: field(map, "kind")?,
                text: field(map, "text")?,
                data: optional(map, "data")?,
            },
            &["t", "kind", "text", "data"][..],
        ),
        "accounts" => (
            Event::Accounts {
                accounts: field(map, "accounts")?,
            },
            &["t", "accounts"][..],
        ),
        "approval" => (
            Event::Approval {
                approval: Approval {
                    id: field(map, "id")?,
                    session: field(map, "session")?,
                    tool: field(map, "tool")?,
                    args: field(map, "args")?,
                    rule: field(map, "rule")?,
                    state: field(map, "state")?,
                    why: field(map, "why")?,
                    deadline: field(map, "deadline")?,
                    extra: BTreeMap::new(),
                },
            },
            &[
                "t", "id", "session", "tool", "args", "rule", "state", "why", "deadline",
            ][..],
        ),
        "job" => (
            Event::Job {
                phase: field(map, "phase")?,
                job: field(map, "job")?,
            },
            &["t", "phase", "job"][..],
        ),
        "shell" => (
            Event::Shell {
                cmd: field(map, "cmd")?,
                output: field(map, "output")?,
                code: field(map, "code")?,
                secs: field(map, "secs")?,
            },
            &["t", "cmd", "output", "code", "secs"][..],
        ),
        "deleted" => (Event::Deleted {}, &["t"][..]),
        _ => {
            return Err(<serde_json::Error as serde::de::Error>::custom(
                "unknown event kind",
            ));
        }
    };
    Ok(result)
}
impl<'de> Deserialize<'de> for Event {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        let map = value
            .as_object()
            .ok_or_else(|| D::Error::custom("event must be an object"))?;
        decode(map).map(|x| x.0).map_err(D::Error::custom)
    }
}
impl<'de> Deserialize<'de> for EventRecord {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        let map = value
            .as_object()
            .ok_or_else(|| D::Error::custom("event must be an object"))?;
        let ts = field(map, "ts").map_err(D::Error::custom)?;
        let (event, keys) = decode(map).map_err(D::Error::custom)?;
        let extra = map
            .iter()
            .filter(|(key, _)| key.as_str() != "ts" && !keys.contains(&key.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        Ok(Self { ts, event, extra })
    }
}
impl<'de> Deserialize<'de> for StreamEvent {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut value = Value::deserialize(deserializer)?;
        let map = value
            .as_object_mut()
            .ok_or_else(|| D::Error::custom("stream event must be an object"))?;
        let sid = field(map, "sid").map_err(D::Error::custom)?;
        map.remove("sid");
        let record = serde_json::from_str(&value.to_string()).map_err(D::Error::custom)?;
        Ok(Self { sid, record })
    }
}
