//! Proves synthetic verification, auth failures, and account isolation lifecycle.
use crate::{setup, support::Server};
use serde_json::json;
use std::{
    fs, thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[test]
fn verification_probe_parks_and_then_restores_the_signed_in_account() {
    let mut server = Server::new_stopped("verification", true);
    let url = "https://accounts.google.com/signin/continue?fixture=1";
    setup::script(
        &server,
        json!({"metadata_by_slot":{setup::ONE:{"verify_url":url}}}),
    );
    setup::start(&mut server);
    let response = server.call(
        "POST",
        "/api/accounts/one@example.com/check",
        Some(json!({})),
    );
    assert_eq!(response.status, 200);
    let accounts = response.json();
    assert_eq!(accounts[0]["verify"], url);
    assert!(accounts[0]["walls"]["gemini"].as_f64().unwrap() > 0.0);
    assert!(accounts[0]["walls"]["cg"].as_f64().unwrap() > 0.0);
    setup::script(&server, json!({}));
    let response = server.call(
        "POST",
        "/api/accounts/one@example.com/check",
        Some(json!({})),
    );
    assert_eq!(response.status, 200);
    assert_eq!(response.json()[0]["verify"], serde_json::Value::Null);
    assert_eq!(response.json()[0]["walls"], json!({"gemini":0.0,"cg":0.0}));
    setup::evidence(
        "verification",
        json!({"urlPreserved":true,"bothBucketsParked":true,"successfulProbeClearsVerification":true}),
    );
    server.stop();
}

#[test]
fn quota_cache_preserves_unknown_fields_and_disabled_window_uses_weekly_reset() {
    let mut server = Server::new_stopped("quota-cache", true);
    let cache = server.data.join("state/quota/one@example.com.json");
    let mut saved: serde_json::Value = serde_json::from_slice(&fs::read(&cache).unwrap()).unwrap();
    saved["future"] = json!({"opaque":123456789012345678901234567890_u128});
    setup::write(&cache, &saved.to_string());
    let quota = json!({"status":"SUCCESS","command":{"name":"quota","data":{"groups":[
        {"name":"Gemini","buckets":[{"window":"5h","disabled":true},{"window":"weekly","remaining_fraction":0,"reset_time":"2099-01-07T00:00:00Z"}]},
        {"name":"Claude/GPT","buckets":[{"window":"5h","remaining_fraction":0.9,"reset_time":"2099-01-01T05:00:00Z"}]}
    ]}}});
    setup::script(
        &server,
        json!({"metadata_by_slot":{setup::ONE:{"quota":quota}}}),
    );
    setup::start(&mut server);
    let response = server.call(
        "POST",
        "/api/accounts/one@example.com/check",
        Some(json!({})),
    );
    assert_eq!(response.status, 200);
    let account = &response.json()[0];
    assert_eq!(account["quota"]["gemini"]["5h"], json!([0.0, null]));
    assert_eq!(account["walls"]["gemini"], 4071427320.0);
    assert_eq!(account["walls"]["cg"], 0.0);
    let saved_after: serde_json::Value = serde_json::from_slice(&fs::read(cache).unwrap()).unwrap();
    assert_eq!(saved_after["future"], saved["future"]);
    setup::evidence(
        "quota-cache",
        json!({"unknownFieldsPreserved":true,"disabledWindow":[0.0,null],"weeklyWall":4071427320.0}),
    );
    server.stop();
}

#[test]
fn stop_cancels_an_account_lease_wait_and_releases_admission() {
    let mut server = Server::new_stopped("lease-wait", true);
    setup::start(&mut server);
    let lease = setup::lease(&server, setup::ONE);
    let paused = server.call(
        "POST",
        "/api/accounts/two@example.com/pause",
        Some(json!({"on":true})),
    );
    assert_eq!(paused.status, 200);
    let id = server.create();
    setup::send(&server, &id, "Lease wait cancellation.");
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let meta = server
            .call("GET", &format!("/api/sessions/{id}"), None)
            .json();
        if meta["meta"]["status"] == "waiting" {
            break;
        }
        assert!(Instant::now() < deadline, "waiting not observed: {meta}");
        thread::sleep(Duration::from_millis(10));
    }
    let started = Instant::now();
    assert_eq!(
        server
            .call("POST", &format!("/api/sessions/{id}/stop"), Some(json!({})))
            .status,
        200
    );
    loop {
        let meta = server
            .call("GET", &format!("/api/sessions/{id}"), None)
            .json();
        if meta["meta"]["status"] == "idle" {
            break;
        }
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "stop did not cancel lease wait: {meta}"
        );
        thread::sleep(Duration::from_millis(10));
    }
    assert!(setup::turn_calls(&server).is_empty());
    drop(lease);
    setup::send(&server, &id, "Run after cancellation.");
    let completed = server.idle(&id);
    assert_eq!(completed["meta"]["turns"], 1);
    setup::evidence(
        "lease-wait-cancel",
        json!({"cancelledBeforeRunner":true,"boundedSeconds":2,"nextPromptCompletes":true}),
    );
    server.stop();
}

#[test]
fn auth_failure_parks_both_baskets_then_hands_off_without_token_changes() {
    let mut server = Server::new_stopped("auth-handoff", true);
    setup::script(&server, json!({"scenarios_by_slot":{setup::ONE:"auth"}}));
    let auth = server
        .data
        .join("slots/one@example.com/.gemini/antigravity-cli/antigravity-oauth-token");
    let token = fs::read(&auth).unwrap();
    setup::start(&mut server);
    let id = server.create();
    setup::send(&server, &id, "Authentication handoff.");
    let completed = server.idle_turns(&id, 2);
    let accounts = server.call("GET", "/api/accounts", None).json();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();
    for bucket in ["gemini", "cg"] {
        assert!(accounts[0]["walls"][bucket].as_f64().unwrap() > now + 29.0 * 86400.0);
        assert_eq!(accounts[0]["why"][bucket], "signed out: sign in again");
    }
    assert_eq!(completed["meta"]["last_slot"], setup::TWO);
    assert_eq!(fs::read(auth).unwrap(), token);
    let calls =
        crate::handoff::assert_handoff_continuation(&server, &completed, "Authentication handoff.");
    assert!(
        calls
            .iter()
            .all(|row| row["ssh_connection_present"] == true)
    );
    setup::evidence(
        "auth-handoff",
        json!({"bothBucketsParked":true,"tokenPreserved":true,"nextAccount":setup::TWO,"sameStoredConversation":true,"continuationNudges":1,"originalUserEvents":1,"privateHomes":calls.iter().map(|row|row["home"].clone()).collect::<Vec<_>>()}),
    );
    server.stop();
}

#[test]
fn stop_cancels_a_slow_quota_probe_and_frees_the_account_lease() {
    let mut server = Server::new_stopped("probe-cancel", true);
    setup::start(&mut server);
    let id = server.create();
    setup::script(
        &server,
        json!({"scenarios_by_slot":{setup::ONE:"quota"},"metadata_by_slot":{setup::ONE:{"quota_delay_ms":10000}}}),
    );
    setup::send(&server, &id, "Cancel during quota read.");
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let calls = setup::calls(&server);
        if calls
            .iter()
            .any(|row| setup::argument(row, "-p") == Some("/quota"))
            && server.call("GET", "/api/accounts", None).json()[0]["busy"] == true
        {
            break;
        }
        assert!(Instant::now() < deadline, "quota probe not observed");
        thread::sleep(Duration::from_millis(10));
    }
    let stopped = Instant::now();
    assert_eq!(
        server
            .call("POST", &format!("/api/sessions/{id}/stop"), Some(json!({})))
            .status,
        200
    );
    loop {
        let session = server
            .call("GET", &format!("/api/sessions/{id}"), None)
            .json();
        let accounts = server.call("GET", "/api/accounts", None).json();
        if session["meta"]["status"] == "idle" && accounts[0]["busy"] == false {
            break;
        }
        assert!(
            stopped.elapsed() < Duration::from_secs(2),
            "quota probe cancellation blocked: {session} {accounts}"
        );
        thread::sleep(Duration::from_millis(10));
    }
    setup::script(&server, json!({}));
    setup::send(&server, &id, "Continue after cancelled quota read.");
    let completed = server.idle_turns(&id, 2);
    assert_eq!(completed["meta"]["last_slot"], setup::ONE);
    setup::evidence(
        "probe-cancel",
        json!({"boundedSeconds":2,"leaseFreed":true,"nextTurnCompletes":true,"cancelledProbeDoesNotWall":true}),
    );
    server.stop();
}
