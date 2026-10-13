//! Proves that sessions share accounts and that placement fills or spreads them.
use crate::{setup, support::Server};
use serde_json::{Value, json};
use std::{
    thread,
    time::{Duration, Instant},
};

fn running_calls(server: &Server, count: usize) -> Vec<Value> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let calls = setup::turn_calls(server);
        if calls.len() >= count {
            return calls;
        }
        assert!(
            Instant::now() < deadline,
            "only {} calls started",
            calls.len()
        );
        thread::sleep(Duration::from_millis(10));
    }
}
fn status(server: &Server, id: &str) -> Value {
    server
        .call("GET", &format!("/api/sessions/{id}"), None)
        .json()["meta"]["status"]
        .clone()
}

#[test]
fn fill_first_runs_several_sessions_on_one_account_at_once() {
    let mut server = Server::new_stopped("fill-first", true);
    setup::script(&server, json!({"delay_ms":1500}));
    setup::start(&mut server);
    let ids: Vec<_> = (0..3).map(|_| server.create()).collect();
    for (i, id) in ids.iter().enumerate() {
        setup::send(&server, id, &format!("Fill request {i}."));
        running_calls(&server, i + 1);
    }
    let calls = running_calls(&server, 3);
    // All three calls started before the first one ended, on the same account and home.
    assert!(ids.iter().all(|id| status(&server, id) != "idle"));
    assert!(calls.iter().all(|row| row["slot"] == setup::ONE));
    assert!(calls.iter().all(|row| row["home"] == calls[0]["home"]));
    let accounts = server.call("GET", "/api/accounts", None).json();
    assert_eq!(accounts[0]["busy"], true);
    assert_eq!(accounts[1]["busy"], false);
    // A probe still needs the account alone.
    assert_eq!(
        server
            .call(
                "POST",
                &format!("/api/accounts/{}/check", setup::ONE),
                Some(json!({}))
            )
            .status,
        400
    );
    for id in &ids {
        assert_eq!(server.idle(id)["meta"]["last_slot"], setup::ONE);
    }
    setup::wait(&server, "leases released", |rows| {
        rows.as_array()
            .unwrap()
            .iter()
            .all(|row| row["busy"] == false)
    });
    setup::evidence(
        "fill-first",
        json!({"placement":"fill","concurrentSessions":3,"account":setup::ONE,"probeNeedsAccountAlone":true}),
    );
    server.stop();
}

#[test]
fn spread_first_prefers_idle_accounts_then_shares_the_rotation_choice() {
    let mut server = Server::new_stopped("spread-first", true);
    setup::write(
        &server.data.join("settings.json"),
        &json!({"placement":"spread","keep_awake":"off"}).to_string(),
    );
    setup::script(&server, json!({"delay_ms":1500}));
    setup::start(&mut server);
    let ids: Vec<_> = (0..3).map(|_| server.create()).collect();
    for (i, id) in ids.iter().enumerate() {
        setup::send(&server, id, &format!("Spread request {i}."));
        running_calls(&server, i + 1);
    }
    let calls = running_calls(&server, 3);
    assert!(ids.iter().all(|id| status(&server, id) != "idle"));
    let slots: Vec<_> = calls.iter().map(|row| row["slot"].clone()).collect();
    // Idle accounts first; with both in use, the rotation order picks the first account again.
    assert_eq!(
        slots,
        [json!(setup::ONE), json!(setup::TWO), json!(setup::ONE)]
    );
    for id in &ids {
        server.idle(id);
    }
    setup::evidence("spread-first", json!({"placement":"spread","slots":slots}));
    server.stop();
}

#[test]
fn placement_setting_saves_spread_and_keeps_fill_out_of_the_wire_format() {
    let mut server = Server::new_stopped("placement-setting", true);
    setup::start(&mut server);
    let put = |value: Value| {
        server.call(
            "PUT",
            "/api/settings/defaults",
            Some(json!({ "placement": value })),
        )
    };
    let spread = put(json!("spread"));
    assert_eq!(spread.status, 200);
    assert_eq!(spread.json()["placement"], "spread");
    let saved: Value =
        serde_json::from_slice(&std::fs::read(server.data.join("settings.json")).unwrap()).unwrap();
    assert_eq!(saved["placement"], "spread");
    assert_eq!(
        server.call("GET", "/api/settings", None).json()["defaults"]["placement"],
        "spread"
    );
    let rejected = put(json!("both"));
    assert_eq!(rejected.status, 400);
    assert_eq!(
        rejected.json(),
        json!({"error":"placement: fill or spread"})
    );
    let fill = put(json!("fill"));
    assert_eq!(fill.status, 200);
    assert!(fill.json().get("placement").is_none());
    assert!(
        server.call("GET", "/api/settings", None).json()["defaults"]
            .get("placement")
            .is_none()
    );
    server.stop();
}

fn quota_calls(server: &Server) -> usize {
    setup::calls(server)
        .iter()
        .filter(|call| setup::argument(call, "-p") == Some("/quota"))
        .count()
}

fn stale_reading(server: &Server) {
    let path = server.data.join(format!("state/quota/{}.json", setup::ONE));
    let mut reading: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    reading["at"] = json!(0);
    setup::write(&path, &reading.to_string());
}

#[test]
fn post_turn_probes_defer_while_another_model_lease_is_held() {
    for quota_error in [false, true] {
        let mut server = Server::new_stopped("post-turn-exclusive-probe", true);
        setup::start(&mut server);
        stale_reading(&server);
        let held = antiquota_store::lock_file_shared(
            &server
                .data
                .join(format!("state/leases/{}.lock", setup::ONE)),
        )
        .unwrap()
        .unwrap();
        if quota_error {
            setup::script(&server, json!({"scenarios_by_slot":{setup::ONE:"quota"}}));
        }
        let before = quota_calls(&server);
        let id = server.create();
        setup::send(&server, &id, "Probe only after every shared call ends.");
        let done = server.idle_turns(&id, if quota_error { 2 } else { 1 });
        assert_eq!(
            quota_calls(&server),
            before,
            "no quota probe may overlap another shared call; quota error: {quota_error}"
        );
        if quota_error {
            assert_eq!(done["meta"]["last_slot"], setup::TWO);
            let accounts = server.call("GET", "/api/accounts", None).json();
            assert!(accounts[0]["walls"]["gemini"].as_f64().unwrap() > 0.0);
            assert_eq!(accounts[0]["why"]["gemini"], "quota error");
        }
        drop(held);
        assert_eq!(
            server
                .call(
                    "POST",
                    "/api/accounts/one@example.com/check",
                    Some(json!({}))
                )
                .status,
            200
        );
        assert_eq!(quota_calls(&server), before + 1);
        server.stop();
    }
}

#[test]
fn automatic_probes_exclude_new_shared_calls_until_the_reading_is_saved() {
    for quota_error in [false, true] {
        let mut server = Server::new_stopped("automatic-probe-exclusive", true);
        setup::start(&mut server);
        stale_reading(&server);
        setup::script(
            &server,
            json!({
                "scenarios_by_slot":{setup::ONE:if quota_error { "quota" } else { "success" }},
                "metadata_by_slot":{setup::ONE:{"quota_delay_ms":500}}
            }),
        );
        let before = quota_calls(&server);
        let id = server.create();
        setup::send(&server, &id, "Hold the probe lease until publication.");
        let deadline = Instant::now() + Duration::from_secs(5);
        while quota_calls(&server) == before {
            assert!(Instant::now() < deadline, "automatic probe never started");
            thread::sleep(Duration::from_millis(5));
        }
        let lock = server
            .data
            .join(format!("state/leases/{}.lock", setup::ONE));
        assert!(
            antiquota_store::lock_file_shared(&lock).unwrap().is_none(),
            "automatic probes must exclude a new shared model call; quota error: {quota_error}"
        );
        server.idle_turns(&id, if quota_error { 2 } else { 1 });
        assert!(antiquota_store::lock_file_shared(&lock).unwrap().is_some());
        server.stop();
    }
}

#[test]
fn periodic_refresh_reads_busy_accounts_under_a_shared_lease_but_leaves_walled_ones() {
    let mut server = Server::new_stopped("busy-account-refresh", true);
    setup::start(&mut server);
    let lock = server
        .data
        .join(format!("state/leases/{}.lock", setup::ONE));
    // An agent's model call holds the account for the whole test.
    let held = antiquota_store::lock_file_shared(&lock).unwrap().unwrap();
    stale_reading(&server);
    let before = quota_calls(&server);
    assert_eq!(
        server
            .call("POST", "/api/accounts/probe", Some(json!({})))
            .status,
        200
    );
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let accounts = server.call("GET", "/api/accounts", None).json();
        if accounts[0]["probed"].as_f64().unwrap_or(0.0) > 0.0 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "busy account was never refreshed"
        );
        thread::sleep(Duration::from_millis(20));
    }
    assert!(quota_calls(&server) > before);
    // A walled busy account keeps its wall until an exclusive probe can run.
    let until = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 3600) as f64;
    let wall = server
        .data
        .join(format!("state/walls/{}.gemini", setup::ONE));
    setup::write(&wall, &format!("{until} quota error\n"));
    stale_reading(&server);
    assert_eq!(
        server
            .call("POST", "/api/accounts/probe", Some(json!({})))
            .status,
        200
    );
    thread::sleep(Duration::from_millis(800));
    let accounts = server.call("GET", "/api/accounts", None).json();
    assert_eq!(
        accounts[0]["probed"].as_f64(),
        Some(0.0),
        "walled busy account is not read"
    );
    assert!(accounts[0]["walls"]["gemini"].as_f64().unwrap() > 0.0);
    drop(held);
    server.stop();
}
