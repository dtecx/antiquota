//! Exact captured JSON comparisons with explicit identity and numeric pointer rules.
use super::{Server, http, repo};
use serde_json::Value;
use std::{collections::BTreeMap, fs};

pub struct Replay {
    bindings: BTreeMap<String, String>,
    passed: Vec<String>,
}
impl Replay {
    pub fn new(server: &Server) -> Self {
        Self {
            bindings: BTreeMap::from([
                ("{{TOKEN}}".into(), server.token.clone()),
                ("{{PORT}}".into(), server.port.to_string()),
                (
                    "{{TEST_ROOT}}".into(),
                    server.root.to_string_lossy().into_owned(),
                ),
                (
                    "{{REPO_ROOT}}".into(),
                    repo().to_string_lossy().into_owned(),
                ),
            ]),
            passed: Vec::new(),
        }
    }
    pub fn bind_setup(&mut self, placeholder: &str, actual: &str) {
        assert!(!actual.contains("{{"), "bind from an actual setup response");
        assert!(
            self.bindings
                .insert(placeholder.into(), actual.into())
                .is_none(),
            "setup identity already bound"
        );
    }
    pub fn first_turn_events(&self, actual: &[Value]) {
        let fixture: Value = serde_json::from_slice(
            &fs::read(repo().join("tests/fixtures/api/cases/events-sse.json")).unwrap(),
        )
        .unwrap();
        let indices = [7, 10, 12, 13, 14, 15, 16, 17];
        assert_eq!(
            actual.len(),
            indices.len(),
            "captured first-turn SSE event count"
        );
        for (index, actual) in indices.into_iter().zip(actual) {
            let expected = self.substitute(&fixture["response"]["eventSamples"][index]);
            compare(
                &expected,
                actual,
                &format!("/response/eventSamples/{index}"),
                &fixture["comparison"]["dynamicValues"],
            );
        }
        eprintln!("captured events-sse: all eight first-turn samples matched");
    }
    fn substitute(&self, value: &Value) -> Value {
        match value {
            Value::String(text) => {
                let mut result = text.clone();
                for (key, value) in &self.bindings {
                    result = result.replace(key, value);
                }
                Value::String(result)
            }
            Value::Array(items) => {
                Value::Array(items.iter().map(|item| self.substitute(item)).collect())
            }
            Value::Object(object) => Value::Object(
                object
                    .iter()
                    .map(|(k, v)| (k.clone(), self.substitute(v)))
                    .collect(),
            ),
            _ => value.clone(),
        }
    }
    pub fn case(&mut self, server: &Server, name: &str) -> http::Response {
        let root = repo().join("tests/fixtures/api");
        let fixture: Value =
            serde_json::from_slice(&fs::read(root.join(format!("cases/{name}.json"))).unwrap())
                .unwrap();
        let request = self.substitute(&fixture["request"]);
        let mut headers: BTreeMap<String, String> =
            serde_json::from_value(request["headers"].clone()).unwrap();
        let data = if request.get("json").is_some() {
            request["json"].to_string().into_bytes()
        } else if request.get("base64").is_some() {
            decode_base64(request["base64"].as_str().unwrap())
        } else {
            Vec::new()
        };
        if fixture["request"].get("declaredContentLength").is_none() {
            headers.remove("Content-Length");
        }
        let response = http::request(
            server.port,
            request["method"].as_str().unwrap(),
            request["path"].as_str().unwrap(),
            &headers,
            &data,
        );
        assert_eq!(
            response.status,
            fixture["response"]["status"].as_u64().unwrap() as u16,
            "fixture {name}: {}",
            String::from_utf8_lossy(&response.body)
        );
        let manifest: Value =
            serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap();
        let actual = if fixture["response"].get("json").is_some() {
            response.json()
        } else {
            Value::String(String::from_utf8(response.body.clone()).unwrap())
        };
        let envelope = serde_json::json!({"response":{"json":actual}});
        for binding in manifest["bindings"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|b| b["fixture"] == name && b["bindOnly"] != true)
        {
            let pointer = binding["pointer"].as_str().unwrap();
            let value = envelope
                .pointer(pointer)
                .and_then(Value::as_str)
                .expect("binding field");
            self.bindings.insert(
                binding["placeholder"].as_str().unwrap().into(),
                value.into(),
            );
        }
        let mut expected = self.substitute(&fixture["response"]["json"]);
        if let Some(calendar) = fixture["comparison"].get("calendar") {
            assert_eq!(
                calendar,
                "days[*].date is the replay local date minus the requested number of days plus row index; calendar dates shift with the replay clock.",
                "unknown calendar rule"
            );
            let rows = expected["days"].as_array_mut().expect("calendar days");
            let today = chrono::Local::now().date_naive();
            let first = today - chrono::Duration::days(rows.len() as i64 - 1);
            for (index, row) in rows.iter_mut().enumerate() {
                row["date"] =
                    serde_json::json!((first + chrono::Duration::days(index as i64)).to_string());
            }
        }
        if fixture["response"].get("json").is_some() {
            compare(
                &expected,
                &actual,
                "/response/json",
                &fixture["comparison"]["dynamicValues"],
            );
        }
        for name in ["content-type", "cache-control", "x-content-type-options"] {
            if let Some(expected) = fixture["response"]["headers"]
                .as_object()
                .unwrap()
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(name))
                .map(|(_, v)| v.as_str().unwrap())
            {
                assert_eq!(
                    response.headers.get(name).map(String::as_str),
                    Some(expected),
                    "header {name}"
                );
            }
        }
        assert_eq!(
            response
                .headers
                .get("content-length")
                .expect("Content-Length")
                .parse::<usize>()
                .unwrap(),
            response.body.len(),
            "fixture {name}: Content-Length must equal encoded body bytes"
        );
        assert!(
            valid_http_date(response.headers.get("date").expect("HTTP Date")),
            "fixture {name}: valid HTTP date"
        );
        self.passed.push(name.into());
        eprintln!("captured fixture {name}: HTTP {} matched", response.status);
        response
    }
    pub fn evidence(&self, server: &Server) {
        server.evidence("contract",&serde_json::json!({"passedFixtures":self.passed,"routes":"Slice1 only; remaining routes pending","source":"tests/fixtures/api/manifest.json"}));
    }
}
fn escape(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}
pub fn compare(expected: &Value, actual: &Value, path: &str, dynamic: &Value) {
    if let Some(rule) = dynamic.get(path) {
        let number = actual
            .as_f64()
            .unwrap_or_else(|| panic!("{path} must be numeric"));
        assert!(
            number.is_finite() && number >= 0.0,
            "{path} finite nonnegative"
        );
        match rule["type"].as_str() {
            Some("positive-integer") => assert!(
                number > 0.0 && number.fract() == 0.0,
                "{path} positive integer"
            ),
            Some("nonnegative-number") => {}
            other => panic!("{path}: unknown dynamic rule {other:?}"),
        }
        return;
    }
    match (expected, actual) {
        (Value::Object(e), Value::Object(a)) => {
            assert_eq!(
                e.keys().collect::<Vec<_>>(),
                a.keys().collect::<Vec<_>>(),
                "{path}: exact object keys"
            );
            for (key, value) in e {
                compare(value, &a[key], &format!("{path}/{}", escape(key)), dynamic);
            }
        }
        (Value::Array(e), Value::Array(a)) => {
            assert_eq!(e.len(), a.len(), "{path}: exact array length");
            for (i, (ev, av)) in e.iter().zip(a).enumerate() {
                compare(ev, av, &format!("{path}/{i}"), dynamic);
            }
        }
        (Value::Number(e), Value::Number(a)) => assert!(
            super::numbers::equal(e, a),
            "{path}: exact numeric value {e} != {a}"
        ),
        _ => assert_eq!(expected, actual, "{path}: exact captured value"),
    }
}

/// Decodes recorded raw request bytes without changing malformed JSON fixtures.
fn decode_base64(text: &str) -> Vec<u8> {
    assert!(text.len().is_multiple_of(4), "base64 block length");
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = Vec::new();
    for block in text.as_bytes().as_chunks::<4>().0 {
        let mut bits = 0_u32;
        let mut padding = 0;
        for byte in block {
            bits <<= 6;
            if *byte == b'=' {
                padding += 1;
            } else {
                assert_eq!(padding, 0, "base64 padding must be trailing");
                bits |= alphabet
                    .iter()
                    .position(|v| v == byte)
                    .expect("base64 alphabet") as u32;
            }
        }
        assert!(padding <= 2, "base64 padding count");
        result.extend_from_slice(&bits.to_be_bytes()[1..4 - padding]);
    }
    result
}

/// Checks the IMF-fixdate HTTP date format and Gregorian calendar ranges.
fn valid_http_date(text: &str) -> bool {
    let parts: Vec<_> = text.split(' ').collect();
    if parts.len() != 6
        || parts[5] != "GMT"
        || !["Mon,", "Tue,", "Wed,", "Thu,", "Fri,", "Sat,", "Sun,"].contains(&parts[0])
    {
        return false;
    }
    let Some(month) = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ]
    .iter()
    .position(|v| *v == parts[2]) else {
        return false;
    };
    if parts[1].len() != 2 || parts[3].len() != 4 {
        return false;
    }
    let (Ok(day), Ok(year)) = (parts[1].parse::<usize>(), parts[3].parse::<usize>()) else {
        return false;
    };
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let time: Vec<_> = parts[4].split(':').map(str::parse::<u8>).collect();
    let date_digits = parts[1].bytes().all(|b| b.is_ascii_digit())
        && parts[3].bytes().all(|b| b.is_ascii_digit());
    let time_digits = parts[4].len() == 8
        && parts[4].bytes().enumerate().all(|(i, b)| {
            if i == 2 || i == 5 {
                b == b':'
            } else {
                b.is_ascii_digit()
            }
        });
    let calendar_year = year.saturating_sub(usize::from(month < 2));
    let weekday = (calendar_year + calendar_year / 4 - calendar_year / 100
        + calendar_year / 400
        + [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4][month]
        + day)
        % 7;
    parts[1].len() == 2
        && parts[3].len() == 4
        && year >= 1601
        && date_digits
        && time_digits
        && parts[0] == ["Sun,", "Mon,", "Tue,", "Wed,", "Thu,", "Fri,", "Sat,"][weekday]
        && day > 0
        && day <= days[month]
        && parts[4].len() == 8
        && time.len() == 3
        && time.iter().all(Result::is_ok)
        && time[0].as_ref().is_ok_and(|v| *v < 24)
        && time[1].as_ref().is_ok_and(|v| *v < 60)
        && time[2].as_ref().is_ok_and(|v| *v < 60)
}

#[cfg(test)]
mod tests {
    use super::{compare, decode_base64, valid_http_date};
    use serde_json::json;
    #[test]
    fn recorded_raw_bodies_decode_and_http_dates_are_checked() {
        assert_eq!(decode_base64("ew=="), b"{");
        assert_eq!(decode_base64(""), b"");
        assert_eq!(decode_base64("aGVsbG8="), b"hello");
        assert!(valid_http_date("Wed, 07 Oct 2026 13:45:21 GMT"));
        for date in [
            "Wed, 31 Feb 2026 13:45:21 GMT",
            "Wed, 07 Oct 2026 25:45:21 GMT",
            "invalid",
        ] {
            assert!(!valid_http_date(date));
        }
    }
    #[test]
    #[should_panic(expected = "unknown dynamic rule")]
    fn unknown_dynamic_rule_is_never_silently_ignored() {
        compare(
            &json!(1),
            &json!(2),
            "/n",
            &json!({"/n":{"type":"invented"}}),
        );
    }
}
