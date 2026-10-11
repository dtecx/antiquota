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
        let paths = cfg!(windows).then(|| captured_paths(&fixture));
        assert_eq!(
            actual.len(),
            indices.len(),
            "captured first-turn SSE event count"
        );
        for (index, actual) in indices.into_iter().zip(actual) {
            let expected =
                self.substitute(&fixture["response"]["eventSamples"][index], cfg!(windows));
            compare(
                &expected,
                actual,
                &format!("/response/eventSamples/{index}"),
                &fixture["comparison"]["dynamicValues"],
                paths.as_ref(),
            );
        }
        eprintln!("captured events-sse: all eight first-turn samples matched");
    }
    fn substitute(&self, value: &Value, windows_paths: bool) -> Value {
        match value {
            Value::String(text) => {
                let mut result = if windows_paths {
                    windows_template_paths(text)
                } else {
                    text.clone()
                };
                for (key, value) in &self.bindings {
                    result = result.replace(key, value);
                }
                Value::String(result)
            }
            Value::Array(items) => Value::Array(
                items
                    .iter()
                    .map(|item| self.substitute(item, windows_paths))
                    .collect(),
            ),
            Value::Object(object) => Value::Object(
                object
                    .iter()
                    .map(|(k, v)| (k.clone(), self.substitute(v, windows_paths)))
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
        let request = self.substitute(&fixture["request"], false);
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
        let mut expected = self.substitute(&fixture["response"]["json"], cfg!(windows));
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
            let paths = cfg!(windows).then(|| captured_paths(&fixture));
            compare(
                &expected,
                &actual,
                "/response/json",
                &fixture["comparison"]["dynamicValues"],
                paths.as_ref(),
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PathKind {
    Path,
    Executable,
}

const PATH_ROOTS: [&str; 3] = ["{{TEST_ROOT}}", "{{REPO_ROOT}}", "{{WORKTREE_PATH}}"];

/// Captured filesystem suffixes end at whitespace or quotes; leave all other text exact.
fn windows_template_paths(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut rest = text;
    while let Some((at, root)) = PATH_ROOTS
        .iter()
        .filter_map(|root| rest.find(root).map(|at| (at, *root)))
        .min_by_key(|(at, _)| *at)
    {
        let end = at + root.len();
        output.push_str(&rest[..end]);
        rest = &rest[end..];
        if rest.starts_with('/') {
            let end = rest
                .find(|ch: char| ch.is_whitespace() || matches!(ch, '\'' | '"'))
                .unwrap_or(rest.len());
            output.push_str(&rest[..end].replace('/', "\\"));
            rest = &rest[end..];
        }
    }
    output.push_str(rest);
    output
}

/// Only captured filesystem fields rooted in documented path placeholders vary by platform.
fn captured_paths(fixture: &Value) -> BTreeMap<String, PathKind> {
    fn visit(value: &Value, path: &str, paths: &mut BTreeMap<String, PathKind>) {
        match value {
            Value::Object(object) => {
                for (key, value) in object {
                    visit(value, &format!("{path}/{}", escape(key)), paths);
                }
            }
            Value::Array(items) => {
                for (index, value) in items.iter().enumerate() {
                    visit(value, &format!("{path}/{index}"), paths);
                }
            }
            Value::String(text)
                if PATH_ROOTS.iter().any(|root| {
                    text == root
                        || text
                            .strip_prefix(root)
                            .is_some_and(|tail| tail.starts_with('/'))
                }) =>
            {
                let kind = match path.rsplit('/').next() {
                    Some("agy") => PathKind::Executable,
                    Some(
                        "cwd" | "root" | "path" | "base" | "stack" | "repo" | "log" | "TargetFile",
                    ) => PathKind::Path,
                    _ => return,
                };
                paths.insert(path.into(), kind);
            }
            _ => {}
        }
    }
    let mut paths = BTreeMap::new();
    visit(&fixture["response"], "/response", &mut paths);
    paths
}

fn compare(
    expected: &Value,
    actual: &Value,
    path: &str,
    dynamic: &Value,
    windows: Option<&BTreeMap<String, PathKind>>,
) {
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
                compare(
                    value,
                    &a[key],
                    &format!("{path}/{}", escape(key)),
                    dynamic,
                    windows,
                );
            }
        }
        (Value::Array(e), Value::Array(a)) => {
            assert_eq!(e.len(), a.len(), "{path}: exact array length");
            for (i, (ev, av)) in e.iter().zip(a).enumerate() {
                compare(ev, av, &format!("{path}/{i}"), dynamic, windows);
            }
        }
        (Value::Number(e), Value::Number(a)) => assert!(
            super::numbers::equal(e, a),
            "{path}: exact numeric value {e} != {a}"
        ),
        (Value::String(e), Value::String(a))
            if windows.is_some_and(|fields| fields.contains_key(path)) =>
        {
            let kind = windows.unwrap()[path];
            assert_eq!(
                windows_path(e, kind),
                windows_path(a, kind),
                "{path}: exact captured value"
            )
        }
        _ => assert_eq!(expected, actual, "{path}: exact captured value"),
    }
}

fn windows_path(text: &str, kind: PathKind) -> String {
    let text = text
        .strip_prefix(r"\\?\")
        .unwrap_or(text)
        .replace('\\', "/");
    if kind == PathKind::Executable {
        text.strip_suffix(".exe").map(str::to_owned).unwrap_or(text)
    } else {
        text
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
    use super::{PathKind, Replay, captured_paths, compare, decode_base64, valid_http_date};
    use serde_json::json;
    use std::collections::BTreeMap;
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
            None,
        );
    }

    #[test]
    fn windows_normalization_is_limited_to_captured_path_and_executable_pointers() {
        let fixture = json!({"response":{"json":{
            "cwd":"{{TEST_ROOT}}/home/project",
            "agy":"{{TEST_ROOT}}/fake-agy",
            "path":"{{TEST_ROOT}}/project/script.exe",
            "id":"{{TEST_ROOT}}/identity.exe",
            "error":"{{TEST_ROOT}}/missing.exe",
            "text":"{{TEST_ROOT}}/message.exe"
        }}});
        let paths = captured_paths(&fixture);
        assert_eq!(paths.len(), 3);
        assert_eq!(paths["/response/json/cwd"], PathKind::Path);
        assert_eq!(paths["/response/json/agy"], PathKind::Executable);
        assert_eq!(paths["/response/json/path"], PathKind::Path);
        for (pointer, expected, actual) in [
            (
                "/response/json/cwd",
                "C:/test/home/project",
                r"\\?\C:\test\home\project",
            ),
            (
                "/response/json/agy",
                "C:/test/fake-agy",
                r"\\?\C:\test\fake-agy.exe",
            ),
            (
                "/response/json/path",
                "C:/test/project/script.exe",
                r"C:\test\project\script.exe",
            ),
        ] {
            compare(
                &json!(expected),
                &json!(actual),
                pointer,
                &json!({}),
                Some(&paths),
            );
            assert!(
                std::panic::catch_unwind(|| compare(
                    &json!(expected),
                    &json!(actual),
                    pointer,
                    &json!({}),
                    None
                ))
                .is_err(),
                "non-Windows replay must stay exact"
            );
        }
        assert!(
            std::panic::catch_unwind(|| compare(
                &json!("C:/test/project/script"),
                &json!(r"C:\test\project\script.exe"),
                "/response/json/path",
                &json!({}),
                Some(&paths)
            ))
            .is_err(),
            "ordinary paths must retain executable-looking suffixes"
        );
        for pointer in ["id", "error", "text", "title", "output", "diff", "status"] {
            for (expected, actual) in [("message.exe", "message"), (r"literal\n", "literal/n")] {
                assert!(
                    std::panic::catch_unwind(|| compare(
                        &json!(expected),
                        &json!(actual),
                        &format!("/response/json/{pointer}"),
                        &json!({}),
                        Some(&paths)
                    ))
                    .is_err(),
                    "{pointer} must stay exact on Windows"
                );
            }
        }
    }

    #[test]
    fn windows_expected_templates_expand_only_documented_filesystem_suffixes() {
        let replay = Replay {
            bindings: BTreeMap::from([
                ("{{TEST_ROOT}}".into(), r"C:\test".into()),
                ("{{REPO_ROOT}}".into(), r"D:\repo".into()),
                ("{{WORKTREE_PATH}}".into(), r"E:\tree".into()),
            ]),
            passed: vec![],
        };
        let fixture = json!({"response":{"json":{
            "error":"not a directory: {{TEST_ROOT}}/home/missing",
            "nested":[{"text":"café '{{TEST_ROOT}}/home/project' \"{{REPO_ROOT}}/docs/guide.md\"\n{{WORKTREE_PATH}}/src/main.rs https://example/a/b a/b literal\\n reply.exe"}]
        }}});
        let actual = json!({
            "error":r"not a directory: C:\test\home\missing",
            "nested":[{"text":"café 'C:\\test\\home\\project' \"D:\\repo\\docs\\guide.md\"\nE:\\tree\\src\\main.rs https://example/a/b a/b literal\\n reply.exe"}]
        });
        let expected = replay.substitute(&fixture["response"]["json"], true);
        assert_eq!(expected, actual);
        let paths = captured_paths(&fixture);
        assert!(paths.is_empty(), "embedded messages must compare exactly");
        compare(
            &expected,
            &actual,
            "/response/json",
            &json!({}),
            Some(&paths),
        );
        // Request and non-Windows substitutions retain the captured slash suffixes.
        assert_eq!(
            replay.substitute(&fixture["response"]["json"], false)["error"],
            r"not a directory: C:\test/home/missing"
        );
        for (from, to) in [
            (r"C:\test\home\project", "C:/test/home/project"),
            ("https://example/a/b", r"https:\\example\a\b"),
            ("a/b literal", r"a\b literal"),
            (r"literal\n", "literal/n"),
            ("reply.exe", "reply"),
            ("café", "cafe"),
        ] {
            let mut changed = actual.clone();
            changed["nested"][0]["text"] = json!(
                actual["nested"][0]["text"]
                    .as_str()
                    .unwrap()
                    .replace(from, to)
            );
            assert_ne!(actual, changed);
            assert!(
                std::panic::catch_unwind(|| compare(
                    &expected,
                    &changed,
                    "/response/json",
                    &json!({}),
                    Some(&paths),
                ))
                .is_err(),
                "actual message change {from:?} must not be normalized"
            );
        }
    }
}
