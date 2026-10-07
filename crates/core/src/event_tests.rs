//! Proves captured wire events and opaque event extensions survive serialization.
use crate::{EventRecord, StreamEvent};
use serde_json::{Value, json};
#[path = "../../../tests/rewrite/support/numbers.rs"]
mod numbers;
#[test]
fn captured_sse_kinds_decode_without_inventing_envelopes() {
    let capture: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/api/cases/events-sse.json"
    ))
    .unwrap();
    let mut kinds = std::collections::BTreeSet::new();
    for raw in capture["response"]["eventSamples"].as_array().unwrap() {
        let event: StreamEvent =
            serde_json::from_str(&serde_json::to_string(raw).unwrap()).unwrap();
        let actual = serde_json::to_value(event).unwrap();
        kinds.insert(raw["t"].as_str().unwrap());
        assert_eq!(
            actual.as_object().unwrap().keys().collect::<Vec<_>>(),
            raw.as_object().unwrap().keys().collect::<Vec<_>>()
        );
        equal_payload(raw, &actual, "$");
        assert_eq!(actual["t"], raw["t"]);
        if raw["t"] == "approval" {
            assert_eq!(actual["rule"], raw["rule"]);
            assert!(!actual.as_object().unwrap().contains_key("approval"));
        }
    }
    assert_eq!(kinds.len(), 14);
    assert_eq!(
        capture["response"]["eventSamples"]
            .as_array()
            .unwrap()
            .len(),
        160
    );
}
#[test]
fn unknown_event_fields_preserve_large_numbers() {
    let raw:Value=serde_json::from_str(r#"{"t":"notice","ts":1,"kind":"info","text":"ok","data":{},"future":123456789012345678901234567890,"decimal":1.123456789012345678901234567890}"#).unwrap();
    let record: EventRecord = serde_json::from_str(&serde_json::to_string(&raw).unwrap()).unwrap();
    let result = serde_json::to_value(record).unwrap();
    assert_eq!(result["future"], raw["future"]);
    assert_eq!(result["decimal"], raw["decimal"]);
    assert_eq!(result["data"], json!({}));
}
#[test]
fn known_float_fields_roundtrip_without_changing_bits() {
    let record = EventRecord::new(
        1791380724.634089,
        crate::Event::TurnEnd {
            turn: 1,
            slot: "a".into(),
            status: "SUCCESS".into(),
            usage: Default::default(),
            secs: 0.43004703521728516,
        },
    );
    let decoded: EventRecord =
        serde_json::from_str(&serde_json::to_string(&record).unwrap()).unwrap();
    assert_eq!(record, decoded);
}

fn equal_payload(expected: &Value, actual: &Value, path: &str) {
    match (expected, actual) {
        (Value::Object(left), Value::Object(right)) => {
            assert_eq!(
                left.keys().collect::<Vec<_>>(),
                right.keys().collect::<Vec<_>>(),
                "{path}: keys"
            );
            for (key, value) in left {
                equal_payload(value, &right[key], &format!("{path}/{key}"));
            }
        }
        (Value::Array(left), Value::Array(right)) => {
            assert_eq!(left.len(), right.len(), "{path}: array length");
            for (index, (left, right)) in left.iter().zip(right).enumerate() {
                equal_payload(left, right, &format!("{path}/{index}"));
            }
        }
        (Value::Number(left), Value::Number(right)) => {
            assert!(numbers::equal(left, right), "{path}: {left} != {right}")
        }
        _ => assert_eq!(expected, actual, "{path}: exact value"),
    }
}
