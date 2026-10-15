//! Proves goal mode, the check command, the independent verifier, denied-command follow-ups
//! and plan mode through the real engine and fake agy.
use crate::support::{Server, replay::Replay};
use serde_json::{Value, json};
use std::{
    fs, thread,
    time::{Duration, Instant},
};

/// A check that passes once the agent wrote `notes.txt`, in this platform's shell.
const CHECK: &str = if cfg!(windows) {
    "if exist notes.txt (exit 0) else (exit 1)"
} else {
    "test -f notes.txt"
};

fn script(server: &Server, value: Value) {
    fs::write(&server.script, value.to_string()).unwrap();
}

fn session(server: &Server, id: &str) -> Value {
    server
        .call("GET", &format!("/api/sessions/{id}"), None)
        .json()
}

/// Waits until the goal reaches `state` and the session has gone idle.
fn settled(server: &Server, id: &str, state: &str) -> Value {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let value = session(server, id);
        if value["meta"]["goal"]["state"] == state && value["meta"]["status"] == "idle" {
            return value;
        }
        assert!(
            Instant::now() < deadline,
            "goal did not settle as {state}: {value}"
        );
        thread::sleep(Duration::from_millis(30));
    }
}

fn notices<'a>(value: &'a Value, kind: &str) -> Vec<&'a Value> {
    value["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["t"] == "notice" && e["kind"] == kind)
        .collect()
}

fn user_kinds(value: &Value) -> Vec<String> {
    value["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["t"] == "user")
        .map(|e| e["kind"].as_str().unwrap().to_owned())
        .collect()
}

fn calls(server: &Server) -> Vec<Value> {
    fs::read_to_string(server.root.join("fake-calls.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn prompt_of(call: &Value) -> String {
    let argv = call["argv"].as_array().unwrap();
    argv.windows(2)
        .find(|pair| pair[0] == "-p")
        .map(|pair| pair[1].as_str().unwrap().to_owned())
        .unwrap_or_default()
}

fn set_goal(server: &Server, id: &str, body: Value) -> Value {
    let response = server.call("POST", &format!("/api/sessions/{id}/goal"), Some(body));
    assert_eq!(
        response.status,
        200,
        "{}",
        String::from_utf8_lossy(&response.body)
    );
    response.json()
}

#[test]
fn a_goal_runs_its_check_and_verifier_until_met() {
    let mut server = Server::new_stopped("goal-met", false);
    script(&server, json!({"done_after": 2}));
    server.start();
    let id = server.create();
    let mut replay = Replay::new(&server);
    replay.bind_setup("{{SESSION_1}}", &id);
    replay.case(&server, "goal-empty");
    replay.case(&server, "goal-resume-empty");
    let started = set_goal(
        &server,
        &id,
        json!({"objective": "  Complete fixture. ", "check": CHECK, "verifier": true,
               "verifier_model": "gemini-3.8-flash-high", "max_rounds": 3}),
    );
    assert_eq!(
        started["goal"],
        json!({"objective": "Complete fixture.", "check": CHECK, "verifier": true,
               "verifier_model": "gemini-3.8-flash-high", "max_rounds": 3, "round": 0, "state": "active"})
    );
    let done = settled(&server, &id, "met");
    assert_eq!(done["meta"]["goal"]["round"], 2);
    assert_eq!(done["meta"]["turns"], 2);
    assert_eq!(user_kinds(&done), ["goal", "nudge"]);
    let goal_notes: Vec<&str> = notices(&done, "goal")
        .iter()
        .map(|n| n["text"].as_str().unwrap())
        .collect();
    assert_eq!(
        goal_notes,
        [
            "Goal set: Complete fixture.",
            "The agent stopped before claiming the goal is done. Pushing it on (round 1/3).",
            "Goal met."
        ]
    );
    let check = notices(&done, "check");
    assert_eq!(check.len(), 1);
    assert_eq!(check[0]["text"], format!("Check `{CHECK}` exited 0."));
    assert_eq!(
        check[0]["data"],
        json!({"rc": 0, "output": "", "passed": true})
    );
    let verdict = notices(&done, "verdict");
    assert_eq!(verdict.len(), 1);
    assert_eq!(verdict[0]["text"], "All good.");
    assert_eq!(
        verdict[0]["data"],
        json!({"done": true, "missing": [], "slot": "one@example.com", "model": "gemini-3.8-flash-high"})
    );
    assert_eq!(
        notices(&done, "goal").last().unwrap()["data"],
        json!({"met": true})
    );
    let verifier: Vec<Value> = calls(&server)
        .into_iter()
        .filter(|c| prompt_of(c).contains("VERDICT: DONE"))
        .collect();
    assert_eq!(verifier.len(), 1);
    let argv: Vec<&str> = verifier[0]["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a.as_str().unwrap())
        .collect();
    assert!(argv.windows(2).any(|w| w == ["--output-format", "json"]));
    assert!(!argv.contains(&"--dangerously-skip-permissions"));
    assert_eq!(
        verifier[0]["cwd"],
        server.project.to_string_lossy().as_ref()
    );
    assert!(prompt_of(&verifier[0]).contains("\n\nComplete fixture.\n\n"));
    // Clearing keeps the transcript and only removes the goal record.
    replay.case(&server, "goal-delete");
    assert_eq!(session(&server, &id)["meta"]["goal"], Value::Null);
    server.stop();
}

#[test]
fn a_failed_check_skips_the_verifier_and_the_round_limit_abandons() {
    let mut server = Server::new_stopped("goal-check", false);
    script(&server, json!({}));
    server.start();
    let id = server.create();
    let broken = crate::support::shell_line("echo broken; exit 3", "echo broken& exit 3");
    let nl = crate::support::NL;
    let started = set_goal(
        &server,
        &id,
        json!({"objective": "Pass the check.", "check": broken, "max_rounds": 2}),
    );
    // Legacy API defaults: the verifier is on and uses the session model unless one is named.
    assert_eq!(started["goal"]["verifier"], true);
    assert_eq!(started["goal"]["verifier_model"], "");
    let done = settled(&server, &id, "failed");
    assert_eq!(done["meta"]["goal"]["round"], 3);
    let checks = notices(&done, "check");
    assert_eq!(checks.len(), 2);
    assert_eq!(
        checks[0]["data"],
        json!({"rc": 3, "output": format!("broken{nl}"), "passed": false})
    );
    assert!(
        notices(&done, "verdict").is_empty(),
        "no verifier after a failed check"
    );
    assert!(
        calls(&server)
            .iter()
            .all(|c| !prompt_of(c).contains("VERDICT"))
    );
    let nudges: Vec<&Value> = done["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["t"] == "user" && e["kind"] == "nudge")
        .collect();
    assert_eq!(nudges.len(), 2);
    assert!(nudges[0]["text"].as_str().unwrap().contains(&format!(
        "The objective is not met yet.\n\nThe check command `{broken}` exited 3:\nbroken{nl}"
    )));
    assert_eq!(
        notices(&done, "goal").last().unwrap()["text"],
        "Goal abandoned after 2 rounds."
    );
    let defaulted = set_goal(&server, &server.create(), json!({"objective": "Defaults."}));
    assert_eq!(defaulted["goal"]["max_rounds"], 20);
    server.stop();
}

#[test]
fn a_rejected_verdict_feeds_back_and_a_spent_verifier_pool_falls_back() {
    let mut server = Server::new_stopped("goal-verifier", false);
    // The only account reports a spent Claude/GPT pool, so the verifier's model has no quota.
    let pool = |name: &str, left: f64| {
        json!({"name": name, "buckets": [
            {"window": "5h", "remaining_fraction": left, "reset_time": "2099-01-01T05:00:00Z"},
            {"window": "weekly", "remaining_fraction": left, "reset_time": "2099-01-07T00:00:00Z"}
        ]})
    };
    script(
        &server,
        json!({"verdict_done": false, "metadata": {"quota": {"status": "SUCCESS", "command": {
            "name": "quota", "data": {"groups": [pool("Gemini", 0.9), pool("Claude/GPT", 0.0)]}}}}}),
    );
    server.start();
    let id = server.create();
    set_goal(
        &server,
        &id,
        json!({"objective": "Ship tests.", "verifier_model": "claude-sonnet-4-6", "max_rounds": 1}),
    );
    let done = settled(&server, &id, "failed");
    let info: Vec<&str> = notices(&done, "info")
        .iter()
        .map(|n| n["text"].as_str().unwrap())
        .collect();
    assert!(info.contains(
        &"No account has Claude / GPT quota for the verifier (claude-sonnet-4-6); gemini-3.8-flash-high verifies instead."
    ));
    let verdict = notices(&done, "verdict");
    assert_eq!(verdict[0]["data"]["done"], false);
    assert_eq!(verdict[0]["data"]["missing"], json!(["tests are missing"]));
    assert_eq!(verdict[0]["data"]["model"], "gemini-3.8-flash-high");
    let nudge = done["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["t"] == "user" && e["kind"] == "nudge")
        .unwrap();
    assert!(nudge["text"].as_str().unwrap().contains(
        "An independent verifier says the objective is not met: - tests are missing\n- tests are missing"
    ));
    server.stop();
}

#[test]
fn stop_pauses_a_goal_and_resume_restarts_it() {
    let mut server = Server::new_stopped("goal-resume", false);
    script(&server, json!({"delay_ms": 2000}));
    server.start();
    let id = server.create();
    set_goal(
        &server,
        &id,
        json!({"objective": "Finish.", "verifier": false}),
    );
    thread::sleep(Duration::from_millis(400));
    server.call("POST", &format!("/api/sessions/{id}/stop"), Some(json!({})));
    let paused = settled(&server, &id, "paused");
    assert_eq!(paused["meta"]["goal"]["round"], 0);
    script(&server, json!({}));
    let mut replay = Replay::new(&server);
    replay.bind_setup("{{SESSION_1}}", &id);
    replay.case(&server, "goal-resume");
    let done = settled(&server, &id, "met");
    assert_eq!(done["meta"]["goal"]["round"], 1);
    let resumed = done["events"]
        .as_array()
        .unwrap()
        .iter()
        .rfind(|e| e["t"] == "user")
        .unwrap();
    assert_eq!(resumed["kind"], "nudge");
    assert_eq!(
        resumed["text"],
        "Continue working toward the objective: Finish.\nEnd with GOAL_COMPLETE only when it is fully achieved."
    );
    server.stop();
}

#[test]
fn denied_commands_get_a_bounded_follow_up_outside_goals() {
    let mut server = Server::new_stopped("goal-denied", false);
    let settings = server
        .data
        .join("stack/.gemini/antigravity-cli/settings.json");
    fs::create_dir_all(settings.parent().unwrap()).unwrap();
    fs::write(
        &settings,
        json!({"permissions": {"deny": ["command(rm)"]}}).to_string(),
    )
    .unwrap();
    script(
        &server,
        json!({
            "scenario": "hook",
            "hook_command": [crate::support::binary("antiquota"), "hook", "pre-tool"],
            "hook_payload": {"toolCall": {"name": "run_command", "args": {"CommandLine": "rm -rf build"}}},
        }),
    );
    server.start();
    let id = server.create();
    server.call(
        "POST",
        &format!("/api/sessions/{id}/send"),
        Some(json!({"text": "Clean up."})),
    );
    server.idle_turns(&id, 3);
    thread::sleep(Duration::from_millis(500));
    let done = {
        let later = session(&server, &id);
        assert_eq!(later["meta"]["turns"], 3, "two follow-ups at most: {later}");
        later
    };
    assert_eq!(user_kinds(&done), ["user", "nudge", "nudge"]);
    let denied = notices(&done, "denied");
    assert_eq!(denied.len(), 3);
    assert_eq!(
        denied[0]["data"]["commands"],
        json!(["touch fake-hook-marker"])
    );
    assert_eq!(denied[0]["data"]["rules"], json!(["command(touch)"]));
    let follow = done["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["t"] == "user" && e["kind"] == "nudge")
        .unwrap();
    assert!(follow["text"].as_str().unwrap().starts_with(
        "These commands were denied: headless agy cannot ask for permission, so the turn ended."
    ));
    assert!(
        follow["text"]
            .as_str()
            .unwrap()
            .ends_with("`touch fake-hook-marker`\n\nContinue the task.")
    );
    server.stop();
}

#[test]
fn a_failed_write_tool_streams_and_persists_an_edit_only_denial_notice() {
    let mut server = Server::new_stopped("notice-write-denied", false);
    let path = server.project.join("protected.txt");
    fs::write(&path, "User content must survive.").unwrap();
    let tool = |state: &str| {
        let mut event = json!({"event":"step_update","step_update":{
            "conversation_id":"notice-write-fixture", "step_index":1, "step_type":"tool", "state":state,
            "tool_info":{"name":"replace_file_content","parameters":{"TargetFile":path,"ReplacementContent":"Refused edit"}}
        }});
        if state == "ERROR" {
            event["step_update"]["duration_seconds"] = json!(0.1);
            event["step_update"]["tool_info"]["error"] =
                json!({"type":"TOOL_ERROR","message":"user denied permission to edit this file"});
        }
        event
    };
    script(
        &server,
        json!({"write_notes":false,"exit_code":0,"actions":[
            {"event":{"event":"init","conversation_id":"notice-write-fixture"}},
            {"event":tool("ACTIVE")},
            {"delay_ms":100,"event":tool("ERROR")},
            {"event":{"event":"result","result":{"conversation_id":"notice-write-fixture","status":"SUCCESS","response":"The edit was not performed."}}}
        ]}),
    );
    server.start();
    let id = server.create();
    let mut events = crate::support::http::Events::open(server.port, &server.token);
    assert_eq!(
        server
            .call(
                "POST",
                &format!("/api/sessions/{id}/send"),
                Some(json!({"text":"Try the protected edit."}))
            )
            .status,
        200
    );
    let mut saw_running = false;
    let denial = loop {
        let event = events.next();
        if event["t"] == "tool"
            && event["name"] == "replace_file_content"
            && event["state"] == "running"
        {
            saw_running = true;
        }
        if event["t"] == "notice" && event["kind"] == "denied" {
            break event;
        }
    };
    assert!(
        saw_running,
        "the running tool must reach the live transcript"
    );
    let expected = json!({"commands":[],"actions":[{"action":"replace_file_content","display_name":"replace_file_content"}],"rules":[]});
    assert_eq!(denial["data"], expected);
    let finished = server.idle(&id);
    assert_eq!(notices(&finished, "denied")[0]["data"], expected);
    assert_eq!(finished["meta"]["turns"], 1);
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "User content must survive."
    );
    assert!(!server.project.join("notes.txt").exists());
    drop(events);
    server.stop();
}

#[test]
fn historical_denial_http_views_vet_suggestions_without_rewriting_jsonl() {
    let mut server = Server::new("notice-history");
    let id = server.create();
    server.stop();
    let path = server.data.join("sessions").join(&id).join("events.jsonl");
    let mut original = fs::read(&path).unwrap();
    let data = json!({
        "commands":["git status", "git status", "/usr/bin/git status --short", "'/bin/bash' -c unsafe", "/usr/bin/python3.11 -c unsafe", "git push --force origin main", "/usr/bin/find . -delete", "/usr/bin/sed -i '' s/a/b/ file", false, null],
        "rules":["command(*)","command(bash)","command(git push)"],
        "actions":[{"action":"write_to_file","display_name":"Historical file edit"}],
        "future":{"retained":true}
    });
    for (ts, data) in [(123, data.clone()), (124, json!(false))] {
        let record =
            json!({"t":"notice","ts":ts,"kind":"denied","text":"Historical denial.","data":data});
        original.extend_from_slice(format!("{record}\n").as_bytes());
    }
    fs::write(&path, &original).unwrap();
    server.start();
    for _ in 0..2 {
        let view = session(&server, &id);
        let denials = notices(&view, "denied");
        assert_eq!(denials.len(), 2);
        let mut expected = data.clone();
        expected["rules"] = json!(["command(git status)", "command(/usr/bin/git status)"]);
        assert_eq!(denials[0]["data"], expected);
        assert_eq!(denials[1]["data"], json!({"rules":[]}));
        assert_eq!(view["meta"]["turns"], 0);
        assert_eq!(
            fs::read(&path).unwrap(),
            original,
            "GET must not rewrite historical bytes"
        );
    }
    server.stop();
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn plan_mode_plans_by_instruction_without_native_plan_mode() {
    let mut server = Server::new("goal-plan");
    let id = server.create();
    let patched = server.call(
        "PATCH",
        &format!("/api/sessions/{id}"),
        Some(json!({"mode": "plan"})),
    );
    assert_eq!(patched.json()["mode"], "plan");
    server.call(
        "POST",
        &format!("/api/sessions/{id}/send"),
        Some(json!({"text": "Add a feature."})),
    );
    server.idle(&id);
    let turn = calls(&server)
        .into_iter()
        .find(|c| {
            c["argv"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a == "stream-json")
        })
        .unwrap();
    assert!(prompt_of(&turn).starts_with("(antiquota plan mode) Investigate as needed"));
    assert!(prompt_of(&turn).ends_with("\n\nAdd a feature."));
    let argv = turn["argv"].as_array().unwrap();
    assert!(!argv.windows(2).any(|w| w[0] == "--mode" && w[1] == "plan"));
    assert!(argv.iter().any(|a| a == "--dangerously-skip-permissions"));
    server.stop();
}

fn goal_cli(server: &Server, extra: &[&str]) -> std::process::Output {
    let mut command = server.command();
    command
        .args(["goal", "Complete fixture.", "--cwd"])
        .arg(&server.project)
        .args(["--check", CHECK])
        .args(extra);
    crate::support::bounded_output(&mut command)
}

#[test]
fn the_goal_command_starts_its_own_engine_and_exits_zero_when_met() {
    let server = Server::new_stopped("goal-cli-own", false);
    script(&server, json!({}));
    let output = goal_cli(&server, &[]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("\n>> goal: Objective: Complete fixture."));
    assert!(stdout.contains("\n-- turn 1 on one@example.com (gemini-3.8-flash-high)"));
    assert!(stdout.contains(&format!("== check: Check `{CHECK}` exited 0.")));
    assert!(stdout.contains("== goal: Goal met."));
    assert!(
        !server.data.join("state/engine.json").exists(),
        "its own engine stopped"
    );
    let sessions = fs::read_dir(server.data.join("sessions")).unwrap().count();
    assert_eq!(sessions, 1, "the session stays for the app");
}

#[test]
fn the_goal_command_uses_the_running_engine_and_fails_when_abandoned() {
    let mut server = Server::new_stopped("goal-cli-shared", false);
    script(&server, json!({"verdict_done": false}));
    server.start();
    let output = goal_cli(&server, &["--max-rounds", "1", "--mode", "default"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(1), "{stdout}");
    assert!(stdout.contains("== goal: Goal abandoned after 1 rounds."));
    let state = server.call("GET", "/api/state", None).json();
    let sessions = state["sessions"].as_array().unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0]["mode"], "default");
    assert_eq!(sessions[0]["goal"]["state"], "failed");
    assert!(
        server.data.join("state/engine.json").exists(),
        "the shared engine keeps running"
    );
    server.stop();
}

#[cfg(unix)]
#[test]
fn ctrl_c_stops_the_goal_command_keeps_the_goal_resumable_and_exits_one() {
    use std::io::Read;
    let mut server = Server::new_stopped("goal-cli-interrupt", false);
    script(&server, json!({"delay_ms": 20_000}));
    server.start();
    let mut child = server
        .command()
        .args(["goal", "Complete fixture.", "--cwd"])
        .arg(&server.project)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    // Read the log as it streams, and interrupt once the first turn has started.
    let mut stdout = child.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    let reader = thread::spawn(move || {
        let mut chunk = [0u8; 4096];
        while let Ok(count) = stdout.read(&mut chunk) {
            if count == 0
                || tx
                    .send(String::from_utf8_lossy(&chunk[..count]).into())
                    .is_err()
            {
                break;
            }
        }
    });
    let mut log = String::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while !log.contains("\n-- turn 1 on one@example.com") {
        assert!(
            std::time::Instant::now() < deadline,
            "no turn started: {log}"
        );
        if let Ok(text) = rx.recv_timeout(Duration::from_millis(50)) {
            log.push_str(&text);
        }
    }
    let sent = std::process::Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status()
        .unwrap();
    assert!(sent.success());
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the goal command kept running"
        );
        thread::sleep(Duration::from_millis(20));
    };
    reader.join().unwrap();
    while let Ok(text) = rx.try_recv() {
        log.push_str(&text);
    }
    assert_eq!(status.code(), Some(1), "{log}");
    let state = server.call("GET", "/api/state", None).json();
    let id = state["sessions"][0]["id"].as_str().unwrap().to_owned();
    assert!(
        log.ends_with(&format!(
            "\nstopped; session {id} can be resumed in the app\n"
        )),
        "{log}"
    );
    let paused = settled(&server, &id, "paused");
    assert_eq!(paused["meta"]["status"], "idle");
    assert_eq!(paused["meta"]["goal"]["objective"], "Complete fixture.");
    assert!(
        server.data.join("state/engine.json").exists(),
        "the shared engine keeps running"
    );
    server.stop();
}
