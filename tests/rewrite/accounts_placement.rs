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
