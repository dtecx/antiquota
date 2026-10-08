//! Owns disposable account setup, bounded observation, and Slice 2 evidence.
use crate::support::{Server, repo};
use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    thread,
    time::{Duration, Instant},
};

pub const ONE: &str = "one@example.com";
pub const TWO: &str = "two@example.com";
pub fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}
pub fn captured_home(server: &Server) {
    for (path, text) in [
        (".gemini/GEMINI.md", "Use the isolated project.\n"),
        (".gemini/config/hooks.json", "{}"),
        (".gemini/config/mcp_config.json", "{\"mcpServers\": {}}"),
        (
            ".gemini/config/skills/example/SKILL.md",
            "---\ndescription: Example skill\n---\nFollow this example.\n",
        ),
        (".gemini/config/rules/example.md", "Keep changes small.\n"),
        (
            ".gemini/config/plugins/example/README.md",
            "Fixture plugin resources.\n",
        ),
        (
            ".gemini/config/config.json",
            "{\"plugins\":{\"example\":{\"enabled\":true}}}",
        ),
        (
            ".gemini/antigravity-cli/settings.json",
            "{\"permissions\": {\"allow\": [\"command(ls)\"], \"deny\": []}}",
        ),
        (
            ".gemini/antigravity-cli/installation_id",
            "fixture-installation",
        ),
    ] {
        write(&server.home.join(path), text);
        let stack_path = path.strip_prefix(".gemini/").unwrap();
        write(&server.data.join("stack/.gemini").join(stack_path), text);
    }
    // The data specimen is later in the capture, after an example hook was added.
    // Recreate the earlier GET settings state with its captured protection hook only.
    let hooks: Value = serde_json::from_slice(
        &fs::read(repo().join("tests/fixtures/data/stack-hooks.json")).unwrap(),
    )
    .unwrap();
    write(
        &server.data.join("stack/.gemini/config/hooks.json"),
        &json!({"antiquota":hooks["antiquota"]}).to_string(),
    );
}
pub fn script(server: &Server, data: Value) {
    write(&server.script, &data.to_string());
}
pub fn healthy_quota(fraction: f64) -> Value {
    let groups: Vec<_> = ["Gemini", "Claude/GPT"].into_iter().map(|name| json!({
        "name":name,"buckets":[
            {"window":"5h","remaining_fraction":if name=="Gemini" {fraction} else {0.9},"reset_time":"2099-01-01T05:00:00Z"},
            {"window":"weekly","remaining_fraction":0.9,"reset_time":"2099-01-07T00:00:00Z"}
        ]
    })).collect();
    json!({"status":"SUCCESS","command":{"name":"quota","data":{"groups":groups}}})
}
pub fn wait(server: &Server, label: &str, predicate: impl Fn(&Value) -> bool) -> Value {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let accounts = server.call("GET", "/api/accounts", None);
        assert_eq!(accounts.status, 200);
        let accounts = accounts.json();
        if predicate(&accounts) {
            return accounts;
        }
        assert!(Instant::now() < deadline, "{label}: {accounts}");
        thread::sleep(Duration::from_millis(10));
    }
}
/// Acquire the fixture lease itself; an idle HTTP snapshot is not a lock guarantee.
pub fn lease(server: &Server, slot: &str) -> antiquota_store::FileLease {
    let path = server.data.join(format!("state/leases/{slot}.lock"));
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        if let Some(lease) = antiquota_store::lock_file(&path).unwrap() {
            return lease;
        }
        assert!(
            Instant::now() < deadline,
            "fixture lease remained busy: {slot}"
        );
        thread::sleep(Duration::from_millis(10));
    }
}
#[test]
fn fixture_lease_waits_for_a_real_contending_lock() {
    let server = Server::new_stopped("fixture-lease", true);
    let path = server.data.join(format!("state/leases/{ONE}.lock"));
    let held = antiquota_store::lock_file(&path).unwrap().unwrap();
    let release = thread::spawn(move || {
        thread::sleep(Duration::from_millis(100));
        drop(held);
    });
    let acquired = lease(&server, ONE);
    assert!(antiquota_store::lock_file(&path).unwrap().is_none());
    release.join().unwrap();
    drop(acquired);
    assert!(antiquota_store::lock_file(&path).unwrap().is_some());
}
pub fn calls(server: &Server) -> Vec<Value> {
    let text = fs::read_to_string(server.root.join("fake-calls.jsonl")).unwrap_or_default();
    let complete = text.rfind('\n').map_or("", |end| &text[..end]);
    complete
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
pub fn start(server: &mut Server) {
    let previous = calls(server)
        .iter()
        .filter(|row| row["argv"][0] == "models")
        .count();
    server.start();
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let models = calls(server)
            .iter()
            .filter(|row| row["argv"][0] == "models")
            .count()
            > previous;
        // Overview briefly tests the real lease; do not race the first model acquisition.
        let rows = if models {
            server.call("GET", "/api/accounts", None).json()
        } else {
            Value::Null
        };
        if models
            && rows
                .as_array()
                .unwrap()
                .iter()
                .all(|row| row["busy"] == false)
        {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "startup metadata did not settle: {rows}; calls: {}; engine log: {}",
            serde_json::to_string(&calls(server)).unwrap(),
            fs::read_to_string(server.root.join("server.log")).unwrap_or_default()
        );
        thread::sleep(Duration::from_millis(10));
    }
}
pub fn turn_calls(server: &Server) -> Vec<Value> {
    calls(server)
        .into_iter()
        .filter(|row| {
            row["argv"]
                .as_array()
                .unwrap()
                .windows(2)
                .any(|args| args[0] == "--output-format" && args[1] == "stream-json")
        })
        .collect()
}
pub fn argument<'a>(row: &'a Value, name: &str) -> Option<&'a str> {
    let args = row["argv"].as_array().unwrap();
    args.iter()
        .position(|arg| arg == name)
        .and_then(|i| args.get(i + 1))
        .and_then(Value::as_str)
}
pub fn evidence(name: &str, value: Value) {
    let path = repo().join(format!(".handoff/evidence/slice2-{name}.json"));
    write(&path, &serde_json::to_string_pretty(&value).unwrap());
}
pub fn send(server: &Server, id: &str, prompt: &str) {
    let response = server.call(
        "POST",
        &format!("/api/sessions/{id}/send"),
        Some(json!({"text":prompt})),
    );
    assert_eq!(
        response.status,
        200,
        "{}",
        String::from_utf8_lossy(&response.body)
    );
}
