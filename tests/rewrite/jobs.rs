//! Proves background jobs, loops and daily schedules through the real engine and fake agy.
use crate::support::{Server, replay::Replay};
use serde_json::{Value, json};
use std::{
    fs, thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

fn script(server: &Server, value: Value) {
    fs::write(&server.script, value.to_string()).unwrap();
}

fn rules(server: &Server, permissions: Value) {
    let path = server
        .data
        .join("stack/.gemini/antigravity-cli/settings.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, json!({ "permissions": permissions }).to_string()).unwrap();
}

fn session(server: &Server, id: &str) -> Value {
    server
        .call("GET", &format!("/api/sessions/{id}"), None)
        .json()
}

fn wait_until(
    server: &Server,
    id: &str,
    what: &str,
    seconds: u64,
    test: impl Fn(&Value) -> bool,
) -> Value {
    let deadline = Instant::now() + Duration::from_secs(seconds);
    loop {
        let value = session(server, id);
        if test(&value) {
            return value;
        }
        assert!(Instant::now() < deadline, "{what}: {value}");
        thread::sleep(Duration::from_millis(50));
    }
}

fn events<'a>(value: &'a Value, kind: &str) -> Vec<&'a Value> {
    value["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["t"] == kind)
        .collect()
}

fn job_users(value: &Value) -> Vec<String> {
    events(value, "user")
        .into_iter()
        .filter(|e| e["kind"] == "job")
        .map(|e| e["text"].as_str().unwrap().to_owned())
        .collect()
}

fn send(server: &Server, id: &str, text: &str) {
    let response = server.call(
        "POST",
        &format!("/api/sessions/{id}/send"),
        Some(json!({ "text": text })),
    );
    assert_eq!(response.status, 200);
}

fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
}

#[test]
fn captured_job_loop_and_schedule_routes() {
    let mut server = Server::new("jobs-captured");
    let id = server.create();
    let mut replay = Replay::new(&server);
    replay.bind_setup("{{SESSION_1}}", &id);
    replay.case(&server, "job-empty");
    let created = replay.case(&server, "job-create").json();
    let log = created["log"].as_str().unwrap().to_owned();
    let deadline = Instant::now() + Duration::from_secs(5);
    while fs::read_to_string(&log).unwrap_or_default().is_empty() {
        assert!(Instant::now() < deadline, "job log stays empty");
        thread::sleep(Duration::from_millis(20));
    }
    replay.case(&server, "job-log");
    let state = server.call("GET", "/api/state", None).json();
    assert!(
        state["engine"]["busy"].as_u64().unwrap() >= 1,
        "a running job keeps the engine busy"
    );
    replay.case(&server, "job-stop");
    let stopped = wait_until(&server, &id, "job ends", 5, |v| {
        events(v, "job").iter().any(|e| e["phase"] == "ended")
    });
    let ended = events(&stopped, "job")
        .into_iter()
        .find(|e| e["phase"] == "ended")
        .unwrap()
        .clone();
    assert_eq!(ended["job"]["stopped"], true);
    assert!(
        job_users(&stopped).is_empty(),
        "a stopped silent job sends nothing"
    );
    let again = server.call(
        "POST",
        &format!(
            "/api/sessions/{id}/jobs/{}/stop",
            created["id"].as_str().unwrap()
        ),
        Some(json!({})),
    );
    assert_eq!(again.status, 200);
    assert!(
        again.json()["ended"].as_f64().is_some(),
        "stopping an ended job is harmless"
    );
    replay.case(&server, "loop-invalid");
    replay.case(&server, "schedule-invalid");
    replay.case(&server, "loop-create");
    let looped = session(&server, &id)["meta"]["loop"].clone();
    assert_eq!(looped["every"], 900);
    assert_eq!(looped["prompt"], "Loop fixture.");
    replay.case(&server, "loop-delete");
    assert_eq!(session(&server, &id)["meta"]["loop"], Value::Null);
    let scheduled = replay.case(&server, "schedule-create").json();
    let next = scheduled["next"].as_f64().unwrap();
    assert!(next > now() && next <= now() + 86400.0);
    let daily = session(&server, &id)["meta"]["loop"].clone();
    assert_eq!(
        (
            daily["every"].clone(),
            daily["at"].clone(),
            daily["prompt"].clone()
        ),
        (json!(86400), json!("23:59"), json!("Daily fixture."))
    );
    replay.case(&server, "schedule-delete");
    server.stop();
}

#[test]
fn agent_jobs_pass_the_policy_and_report_back() {
    let mut server = Server::new_stopped("jobs-agent", false);
    rules(
        &server,
        json!({"allow": ["command(printf)"], "deny": ["command(rm)"]}),
    );
    script(
        &server,
        json!({"scenarios_by_turn": ["success", "background"], "background_command": "printf 'bg fixture\\n'",
               "reply": "Example:\n```\nRUN_IN_BACKGROUND: rm -rf /\n```\nDone."}),
    );
    server.start();
    let id = server.create();
    send(&server, &id, "Show an example.");
    let first = server.idle(&id);
    assert!(
        events(&first, "job").is_empty(),
        "an example in a code block never runs"
    );
    send(&server, &id, "Run the job.");
    let done = wait_until(&server, &id, "job result turn", 15, |v| {
        v["meta"]["status"] == "idle" && v["meta"]["turns"] == 3
    });
    let jobs = events(&done, "job");
    assert_eq!(jobs.len(), 2);
    assert_eq!(jobs[0]["phase"], "started");
    assert_eq!(jobs[0]["job"]["by"], "agent");
    assert_eq!(jobs[1]["job"]["code"], 0);
    let result = job_users(&done);
    assert_eq!(result.len(), 1);
    assert!(result[0].starts_with("Background job `printf 'bg fixture\\n'` exit 0 after "));
    assert!(result[0].contains("\n```\nbg fixture\n\n```\nAnalyze the result and continue."));
    server.stop();
}

#[test]
fn unruled_agent_jobs_wait_for_an_answer_and_denied_ones_never_start() {
    let mut server = Server::new_stopped("jobs-ask", false);
    rules(&server, json!({"deny": ["command(rm)"]}));
    script(
        &server,
        json!({"scenario": "background", "background_command": "printf 'asked\\n'", "scenarios_by_turn": ["background", "success", "success"]}),
    );
    server.start();
    let id = server.create();
    send(&server, &id, "Start the job.");
    let parked = wait_until(&server, &id, "background approval", 8, |v| {
        events(v, "approval")
            .iter()
            .any(|e| e["tool"] == "run_in_background" && e["state"] == "pending")
    });
    assert_eq!(parked["meta"]["status"], "approval");
    let card = events(&parked, "approval")[0].clone();
    assert_eq!(card["args"]["CommandLine"], "printf 'asked\\n'");
    assert_eq!(card["rule"], "command(printf)");
    assert!(events(&parked, "job").is_empty());
    let answer = server.call(
        "POST",
        &format!("/api/approvals/{}/answer", card["id"].as_str().unwrap()),
        Some(json!({"decision": "allow"})),
    );
    assert_eq!(answer.json(), json!({"decision": "allow"}));
    let done = wait_until(&server, &id, "approved job result", 15, |v| {
        !job_users(v).is_empty() && v["meta"]["status"] == "idle"
    });
    assert_eq!(events(&done, "job")[0]["job"]["by"], "agent");
    let mut denied = Server::new_stopped("jobs-deny", false);
    rules(&denied, json!({"deny": ["command(rm)"]}));
    script(
        &denied,
        json!({"scenario": "background", "background_command": "rm -rf build"}),
    );
    denied.start();
    let other = denied.create();
    send(&denied, &other, "Clean.");
    let refused = denied.idle(&other);
    assert!(events(&refused, "job").is_empty());
    assert!(events(&refused, "notice").iter().any(|n| n["kind"] == "denied"
        && n["text"] == "A deny rule in antiquota's permissions blocks the background job `rm -rf build`."));
    denied.stop();
    server.stop();
}

#[test]
fn a_goal_waits_for_its_job_without_spending_a_round() {
    let mut server = Server::new_stopped("jobs-goal", false);
    rules(&server, json!({"allow": ["command(sleep)"]}));
    script(
        &server,
        json!({"scenarios_by_turn": ["background", "goal_complete"], "background_command": "sleep 1", "done_after": 9}),
    );
    server.start();
    let id = server.create();
    server.call(
        "POST",
        &format!("/api/sessions/{id}/goal"),
        Some(json!({"objective": "Train.", "verifier": false})),
    );
    let done = wait_until(&server, &id, "goal met after the job", 20, |v| {
        v["meta"]["goal"]["state"] == "met" && v["meta"]["status"] == "idle"
    });
    assert_eq!(
        done["meta"]["goal"]["round"], 1,
        "waiting for the job spends no round"
    );
    assert!(events(&done, "notice").iter().any(|n| n["kind"] == "goal"
        && n["text"]
            == "Waiting for background job `sleep 1`; the goal continues when it finishes."));
    let kinds: Vec<&str> = events(&done, "user")
        .iter()
        .map(|e| e["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["goal", "job"]);
    server.stop();
}

#[test]
fn running_jobs_check_in_while_idle() {
    let mut server = Server::new("jobs-check-in");
    let id = server.create();
    let job = server
        .call(
            "POST",
            &format!("/api/sessions/{id}/jobs"),
            Some(json!({"cmd": "echo working; sleep 7", "every": "1s"})),
        )
        .json();
    assert_eq!(job["every"], 1);
    assert_eq!(job["notify"], true);
    let done = wait_until(&server, &id, "check-in then result", 20, |v| {
        job_users(v).len() >= 2 && v["meta"]["status"] == "idle"
    });
    let users = job_users(&done);
    assert!(users[0].starts_with("Background job `echo working; sleep 7` is still running ("));
    assert!(users[0].contains("\n```\nworking\n\n```\nIf it is going wrong, stop it"));
    assert!(users.last().unwrap().contains("exit 0 after"));
    server.stop();
}

#[test]
fn loops_dispatch_while_idle_and_advance() {
    let mut server = Server::new("jobs-loop");
    let id = server.create();
    let set = server.call(
        "POST",
        &format!("/api/sessions/{id}/loop"),
        Some(json!({"every": "30s", "prompt": "Loop ping."})),
    );
    assert_eq!(set.json(), json!({"ok": true}));
    let before = now();
    let done = wait_until(&server, &id, "loop prompt", 12, |v| {
        events(v, "user").iter().any(|e| e["kind"] == "loop") && v["meta"]["status"] == "idle"
    });
    let looped = &done["meta"]["loop"];
    assert_eq!(looped["every"], 60, "at least a minute between prompts");
    let next = looped["next"].as_f64().unwrap();
    assert!(next >= before + 59.0 && next <= now() + 61.0);
    assert_eq!(events(&done, "user")[0]["text"], "Loop ping.");
    server.stop();
}

#[test]
fn jobs_outlive_the_engine_and_are_adopted_after_a_restart() {
    let mut server = Server::new("jobs-adopt");
    let id = server.create();
    let job = server
        .call(
            "POST",
            &format!("/api/sessions/{id}/jobs"),
            Some(json!({"cmd": "sleep 2; echo survived", "notify": true})),
        )
        .json();
    server.stop();
    server.start();
    let done = wait_until(&server, &id, "adopted job ends", 20, |v| {
        events(v, "job").iter().any(|e| e["phase"] == "ended")
            && !job_users(v).is_empty()
            && v["meta"]["status"] == "idle"
    });
    let ended = events(&done, "job")
        .into_iter()
        .find(|e| e["phase"] == "ended")
        .unwrap();
    assert_eq!(ended["job"]["id"], job["id"]);
    assert_eq!(
        ended["job"]["code"],
        Value::Null,
        "an adopted job's code is unknown"
    );
    assert!(job_users(&done)[0].contains("` finished after "));
    assert!(
        fs::read_to_string(job["log"].as_str().unwrap())
            .unwrap()
            .contains("survived")
    );
    server.stop();
}
