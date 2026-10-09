//! Proves quota walls, buckets, ranking exclusions and genuine error channels through real HTTP.
use crate::{
    setup,
    support::{Server, bounded_output},
};
use serde_json::{Value, json};
use std::{
    fs, thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
}
fn panel(gemini: Value, cg: Value) -> Value {
    json!({"status":"SUCCESS","command":{"name":"quota","data":{"groups":[
        {"name":"Gemini","buckets":gemini},
        {"name":"Claude/GPT","buckets":cg}
    ]}}})
}
fn window(name: &str, fraction: f64, reset: Option<&str>) -> Value {
    let mut row = json!({"window":name,"remaining_fraction":fraction});
    if let Some(reset) = reset {
        row["reset_time"] = json!(reset);
    }
    row
}
fn check(server: &Server, slot: &str) -> Value {
    let response = server.call(
        "POST",
        &format!("/api/accounts/{slot}/check"),
        Some(json!({})),
    );
    assert_eq!(
        response.status,
        200,
        "{}",
        String::from_utf8_lossy(&response.body)
    );
    response
        .json()
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["slot"] == slot)
        .unwrap()
        .clone()
}
fn wall(server: &Server, slot: &str, bucket: &str, until: f64) {
    setup::write(
        &server.data.join(format!("state/walls/{slot}.{bucket}")),
        &format!("{} fixture wall\n", until.floor()),
    );
}
fn create(server: &Server, model: &str) -> String {
    let response = server.call(
        "POST",
        "/api/sessions",
        Some(json!({"cwd":server.project,"title":"Routing fixture","model":model})),
    );
    assert_eq!(response.status, 200);
    response.json()["id"].as_str().unwrap().to_owned()
}
/// Waits until a session is idle again after its wall notice; no turn may have started.
fn settle(server: &Server, id: &str) -> Value {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let value = server
            .call("GET", &format!("/api/sessions/{id}"), None)
            .json();
        if value["meta"]["status"] == "idle"
            && value["events"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["t"] == "notice" && row["kind"] == "wall")
        {
            return value;
        }
        assert!(Instant::now() < deadline, "no wall notice: {value}");
        thread::sleep(Duration::from_millis(20));
    }
}
fn routes(server: &Server) -> Vec<Value> {
    server
        .call("GET", "/api/routing?n=0", None)
        .json()
        .as_array()
        .unwrap()
        .clone()
}

#[test]
fn quota_panel_walls_at_two_percent_with_the_latest_spent_reset_or_the_fallback() {
    const DAY: &str = "2099-01-01T05:00:00Z";
    const WEEK: &str = "2099-01-07T00:00:00Z";
    let mut server = Server::new_stopped("wall-threshold", true);
    setup::script(
        &server,
        json!({"metadata_by_slot":{
            // Gemini: both windows spent; the later weekly reset applies. Claude/GPT: 2.1% is not spent.
            setup::ONE:{"quota":panel(
                json!([window("5h",0.02,Some(DAY)),window("weekly",0.01,Some(WEEK))]),
                json!([window("5h",0.021,Some(DAY)),window("weekly",0.9,Some(WEEK))]))},
            // Gemini: a spent window without any reset falls back to 5 h 10 min.
            setup::TWO:{"quota":panel(
                json!([window("5h",0.0,None)]),
                json!([window("5h",0.9,Some(DAY))]))}
        }}),
    );
    setup::start(&mut server);
    let one = check(&server, setup::ONE);
    assert_eq!(one["quota"]["gemini"]["5h"], json!([0.02, 4070926800.0]));
    assert_eq!(one["walls"]["gemini"], 4071427200.0 + 120.0);
    assert_eq!(one["why"]["gemini"], "quota panel");
    assert_eq!(one["walls"]["cg"], 0.0);
    let before = now().floor();
    let two = check(&server, setup::TWO);
    let after = now();
    let until = two["walls"]["gemini"].as_f64().unwrap();
    assert!(
        until >= before + 18_600.0 + 120.0 - 1.0 && until <= after + 18_600.0 + 120.0,
        "fallback wall {until} outside [{before}, {after}] + 5h10m + 120 s"
    );
    assert_eq!(two["walls"]["cg"], 0.0);
    // A reading just above the threshold clears the earlier wall.
    setup::script(
        &server,
        json!({"metadata":{"quota":panel(
            json!([window("5h",0.0201,Some(DAY)),window("weekly",0.5,Some(WEEK))]),
            json!([window("5h",0.9,Some(DAY))]))}}),
    );
    for slot in [setup::ONE, setup::TWO] {
        let healthy = check(&server, slot);
        assert_eq!(healthy["walls"], json!({"gemini":0.0,"cg":0.0}), "{slot}");
    }
    setup::evidence(
        "wall-threshold",
        json!({"inclusiveTwoPercent":true,"latestSpentReset":4071427320.0,"fallbackSeconds":18720,"aboveThresholdClears":true}),
    );
    server.stop();
}

#[test]
fn model_buckets_keep_independent_walls_and_unwall_clears_both() {
    let mut server = Server::new_stopped("bucket-walls", true);
    setup::start(&mut server);
    let future = now() + 86_400.0;
    // Only the Claude/GPT bucket of the first account is walled.
    wall(&server, setup::ONE, "cg", future);
    let claude = create(&server, "claude-sonnet-fixture");
    setup::send(&server, &claude, "Claude bucket fixture.");
    assert_eq!(server.idle(&claude)["meta"]["last_slot"], setup::TWO);
    let gpt = create(&server, "gpt-fixture");
    setup::send(&server, &gpt, "GPT bucket fixture.");
    assert_eq!(server.idle(&gpt)["meta"]["last_slot"], setup::TWO);
    let gemini = create(&server, "gemini-3.8-flash-high");
    setup::send(&server, &gemini, "Gemini bucket fixture.");
    assert_eq!(server.idle(&gemini)["meta"]["last_slot"], setup::ONE);
    let picks: Vec<_> = routes(&server)
        .into_iter()
        .filter(|row| row["event"] == "pick")
        .map(|row| (row["session"].clone(), row["bucket"].clone()))
        .collect();
    assert_eq!(
        picks,
        [
            (json!(claude), json!("cg")),
            (json!(gpt), json!("cg")),
            (json!(gemini), json!("gemini"))
        ]
    );
    // The raw CLI reads the model from either argument form.
    for model in [
        &["--model", "claude-cli-fixture"][..],
        &["--model=gpt-cli-fixture"][..],
    ] {
        let mut args = vec!["run", "--", "-p", "CLI bucket fixture."];
        args.extend(["--output-format", "stream-json"]);
        args.extend(model);
        let output = bounded_output(server.command().args(&args));
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            setup::turn_calls(&server).last().unwrap()["slot"],
            setup::TWO
        );
    }
    let ledger = fs::read_to_string(server.data.join("state/calls.jsonl")).unwrap();
    let buckets: Vec<Value> = ledger
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap()["bucket"].clone())
        .collect();
    assert_eq!(buckets, [json!("cg"), json!("cg")]);
    // An expired Gemini wall is ignored and is reported as no wall.
    wall(&server, setup::TWO, "gemini", now() - 60.0);
    let accounts = server.call("GET", "/api/accounts", None).json();
    assert_eq!(accounts[1]["walls"]["gemini"], 0.0);
    assert_eq!(accounts[0]["walls"]["cg"], future.floor());
    assert_eq!(accounts[0]["walls"]["gemini"], 0.0);
    // Explicit unwall of one account clears both of its buckets and nothing else.
    wall(&server, setup::ONE, "gemini", future);
    wall(&server, setup::TWO, "cg", future);
    let unwall = bounded_output(server.command().args(["unwall", setup::ONE]));
    assert!(
        unwall.status.success(),
        "{}",
        String::from_utf8_lossy(&unwall.stderr)
    );
    for bucket in ["gemini", "cg"] {
        assert!(
            !server
                .data
                .join(format!("state/walls/{}.{bucket}", setup::ONE))
                .exists()
        );
    }
    let accounts = server.call("GET", "/api/accounts", None).json();
    assert_eq!(accounts[0]["walls"], json!({"gemini":0.0,"cg":0.0}));
    assert_eq!(accounts[1]["walls"]["cg"], future.floor());
    setup::evidence(
        "bucket-walls",
        json!({"claudeAndGptUseCg":true,"cliBothForms":true,"geminiUnaffected":true,"expiredWallIgnored":true,"unwallBothBuckets":true}),
    );
    server.stop();
}

#[test]
fn paused_and_walled_accounts_sit_out_until_resumed_or_expired() {
    let mut server = Server::new_stopped("ranking-exclusions", true);
    setup::write(
        &server.data.join("settings.json"),
        &json!({"rotation":"sticky","keep_awake":"off"}).to_string(),
    );
    setup::start(&mut server);
    let pause = |on: bool| {
        let response = server.call(
            "POST",
            &format!("/api/accounts/{}/pause", setup::ONE),
            Some(json!({"on":on})),
        );
        assert_eq!(response.status, 200);
    };
    let run = |label: &str| {
        let id = server.create();
        setup::send(&server, &id, label);
        server.idle(&id)["meta"]["last_slot"].clone()
    };
    // Sticky routing would always choose the first account by name.
    assert_eq!(run("Sticky baseline."), setup::ONE);
    pause(true);
    assert_eq!(run("First account paused."), setup::TWO);
    pause(false);
    assert_eq!(run("First account resumed."), setup::ONE);
    wall(&server, setup::ONE, "gemini", now() + 3_600.0);
    assert_eq!(run("First account walled."), setup::TWO);
    wall(&server, setup::ONE, "gemini", now() - 1.0);
    assert_eq!(run("First account wall expired."), setup::ONE);
    setup::evidence(
        "ranking-exclusions",
        json!({"paused":setup::TWO,"resumed":setup::ONE,"walled":setup::TWO,"expired":setup::ONE}),
    );
    server.stop();
}

#[test]
fn all_walled_notice_reports_the_earliest_reset_of_the_requested_bucket() {
    let mut server = Server::new_stopped("next-reset", true);
    setup::start(&mut server);
    let base = (now() + 7_200.0).floor();
    wall(&server, setup::ONE, "gemini", base + 900.0);
    wall(&server, setup::TWO, "gemini", base + 300.0);
    // A sooner wall in the other bucket does not count.
    wall(&server, setup::ONE, "cg", base + 10.0);
    let id = server.create();
    setup::send(&server, &id, "Every Gemini account is walled.");
    let value = settle(&server, &id);
    let notice = value["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["t"] == "notice" && row["kind"] == "wall")
        .unwrap()
        .clone();
    assert_eq!(notice["data"]["reset"], base + 300.0);
    assert!(
        notice["text"]
            .as_str()
            .unwrap()
            .starts_with("Every account is out of Gemini quota.")
    );
    assert!(setup::turn_calls(&server).is_empty());
    setup::evidence(
        "next-reset",
        json!({"reset":"earliest live wall of the requested bucket","otherBucketIgnored":true}),
    );
    server.stop();
}

#[test]
fn model_prose_and_successful_stderr_never_wall_an_account() {
    let mut server = Server::new_stopped("genuine-errors", true);
    let prose = "The 429 RESOURCE_EXHAUSTED quota error and UNAUTHENTICATED token expired are only words here; try again later.";
    setup::script(
        &server,
        json!({"reply":prose,"stderr":"warning: rate limit quota 503 UNAVAILABLE chatter"}),
    );
    setup::start(&mut server);
    let id = server.create();
    setup::send(&server, &id, "Explain the errors.");
    let value = server.idle(&id);
    let end = value["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["t"] == "turn_end")
        .unwrap()
        .clone();
    assert_eq!(end["status"], "SUCCESS");
    assert_eq!(value["meta"]["last_slot"], setup::ONE);
    let accounts = server.call("GET", "/api/accounts", None).json();
    for account in accounts.as_array().unwrap() {
        assert_eq!(account["walls"], json!({"gemini":0.0,"cg":0.0}));
    }
    assert!(routes(&server).iter().all(|row| {
        !["wall", "cooldown", "signed-out", "retry"].contains(&row["event"].as_str().unwrap_or(""))
    }));
    assert_eq!(setup::turn_calls(&server).len(), 1);
    setup::evidence(
        "genuine-errors",
        json!({"status":"SUCCESS","walls":0,"handoffs":0,"proseWithErrorWords":true,"stderrChatter":true}),
    );
    server.stop();
}

#[test]
fn an_auth_error_outranks_quota_and_transient_words() {
    let mut server = Server::new_stopped("error-precedence", true);
    let actions = |error: &str| {
        json!([
            {"event":{"event":"init","conversation_id":"precedence-conversation","init":{"model":"stub"}}},
            {"event":{"event":"result","result":{"conversation_id":"precedence-conversation","status":"ERROR","error":error}}}
        ])
    };
    setup::script(
        &server,
        json!({"actions":actions("UNAUTHENTICATED: token expired after RESOURCE_EXHAUSTED 429 and 503 UNAVAILABLE"),"exit_code":1}),
    );
    setup::start(&mut server);
    let id = server.create();
    setup::send(&server, &id, "Auth precedence.");
    settle(&server, &id);
    let accounts = server.call("GET", "/api/accounts", None).json();
    for account in accounts.as_array().unwrap() {
        assert!(
            account["why"]["gemini"]
                .as_str()
                .unwrap()
                .starts_with("signed out"),
            "{account}"
        );
        assert!(account["walls"]["cg"].as_f64().unwrap() > now() + 29.0 * 86_400.0);
    }
    let signed_out: Vec<_> = routes(&server)
        .into_iter()
        .filter(|row| row["event"] == "signed-out")
        .collect();
    assert_eq!(signed_out.len(), 2);
    setup::evidence(
        "error-precedence",
        json!({"authBeatsQuotaAndTransient":true,"bothAccountsParked":true}),
    );
    server.stop();
}

#[test]
fn routing_history_reads_default_to_sixty_and_trim_above_two_megabytes() {
    let mut server = Server::new_stopped("routing-tail", true);
    let line = |i: usize| {
        format!(
            "{}\n",
            json!({"ts":1000.0 + i as f64,"event":"pick","slot":setup::ONE,"bucket":"gemini","why":format!("{i:0>300}")})
        )
    };
    let text: String = (0..6_000).map(line).collect();
    assert!(text.len() > 2_000_000);
    setup::write(&server.data.join("state/routing.jsonl"), &text);
    setup::start(&mut server);
    for (query, count, first) in [
        ("", 60, 5_940),
        ("?n=", 60, 5_940),
        ("?n=5", 5, 5_995),
        ("?n=0", 6_000, 0),
    ] {
        let rows = server.call("GET", &format!("/api/routing{query}"), None);
        assert_eq!(rows.status, 200, "{query}");
        let rows = rows.json();
        let rows = rows.as_array().unwrap();
        assert_eq!(rows.len(), count, "{query}");
        assert_eq!(rows[0]["ts"], 1000.0 + first as f64, "{query}");
    }
    // The next decision appends and keeps only the newest 5000 lines.
    let response = server.call(
        "POST",
        &format!("/api/accounts/{}/pause", setup::ONE),
        Some(json!({"on":true})),
    );
    assert_eq!(response.status, 200);
    let kept = fs::read_to_string(server.data.join("state/routing.jsonl")).unwrap();
    let lines: Vec<_> = kept.lines().collect();
    assert_eq!(lines.len(), 5_000);
    assert_eq!(lines[0], line(1_001).trim_end());
    let last: Value = serde_json::from_str(lines[4_999]).unwrap();
    assert_eq!(last["event"], "paused");
    assert!(kept.len() < 2_000_000);
    setup::evidence(
        "routing-tail",
        json!({"httpDefault":60,"emptyN":60,"trimmedLines":5000,"newestKept":"paused"}),
    );
    server.stop();
}

#[test]
fn removing_an_account_clears_both_of_its_bucket_walls() {
    let mut server = Server::new_stopped("remove-walls", true);
    setup::start(&mut server);
    let future = now() + 86_400.0;
    for slot in [setup::ONE, setup::TWO] {
        for bucket in ["gemini", "cg"] {
            wall(&server, slot, bucket, future);
        }
    }
    let deleted = server.call("DELETE", &format!("/api/accounts/{}", setup::TWO), None);
    assert_eq!(deleted.status, 200);
    for bucket in ["gemini", "cg"] {
        assert!(
            !server
                .data
                .join(format!("state/walls/{}.{bucket}", setup::TWO))
                .exists()
        );
        assert!(
            server
                .data
                .join(format!("state/walls/{}.{bucket}", setup::ONE))
                .exists()
        );
    }
    let accounts = server.call("GET", "/api/accounts", None).json();
    assert_eq!(accounts.as_array().unwrap().len(), 1);
    assert_eq!(accounts[0]["walls"]["cg"], future.floor());
    setup::evidence(
        "remove-walls",
        json!({"removed":setup::TWO,"bothWallsCleared":true,"otherAccountKept":true}),
    );
    server.stop();
}
