//! Proves rotation, reserve and cooldown behavior through real engine turns.
use crate::{setup, support::Server};
use serde_json::{Value, json};
use std::fs;

fn panel(fraction: f64, weekly_reset: &str) -> Value {
    let mut panel = setup::healthy_quota(fraction);
    panel["command"]["data"]["groups"][0]["buckets"][1]["remaining_fraction"] = json!(fraction);
    panel["command"]["data"]["groups"][0]["buckets"][1]["reset_time"] = json!(weekly_reset);
    panel
}

#[test]
fn every_rotation_puts_an_account_below_reserve_after_healthy_accounts() {
    for strategy in ["sticky", "perishable", "round-robin"] {
        let mut server = Server::new_stopped(&format!("reserve-{strategy}"), true);
        setup::write(
            &server.data.join("settings.json"),
            &json!({"rotation":strategy,"reserve":"3","keep_awake":"off"}).to_string(),
        );
        setup::script(
            &server,
            json!({"metadata_by_slot":{
                setup::ONE:{"quota":panel(0.025,"2099-01-07T00:00:00Z")},
                setup::TWO:{"quota":panel(0.8,"2099-01-08T00:00:00Z")}
            }}),
        );
        setup::start(&mut server);
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
        assert_eq!(
            server
                .call(
                    "POST",
                    "/api/accounts/two@example.com/check",
                    Some(json!({}))
                )
                .status,
            200
        );
        let id = server.create();
        setup::send(&server, &id, "Reserve routing fixture.");
        let completed = server.idle(&id);
        assert_eq!(
            completed["meta"]["last_slot"],
            setup::TWO,
            "strategy {strategy}"
        );
        let accounts = server.call("GET", "/api/accounts", None).json();
        assert_eq!(
            accounts[0]["walls"]["gemini"], 0.0,
            "reserve is not a hard wall"
        );
        server.stop();
    }
    setup::evidence(
        "reserve-strategies",
        json!({"strategies":["sticky","perishable","round-robin"],"lowAccount":setup::ONE,"selectedAccount":setup::TWO,"reserveDoesNotWall":true}),
    );
}

#[test]
fn perishable_uses_earliest_weekly_reset_and_round_robin_uses_last_pick() {
    let mut server = Server::new_stopped("perishable-order", true);
    setup::script(
        &server,
        json!({"metadata_by_slot":{
            setup::ONE:{"quota":panel(0.8,"2099-01-08T00:00:00Z")},
            setup::TWO:{"quota":panel(0.5,"2099-01-07T00:00:00Z")}
        }}),
    );
    setup::start(&mut server);
    for account in [setup::ONE, setup::TWO] {
        assert_eq!(
            server
                .call(
                    "POST",
                    &format!("/api/accounts/{account}/check"),
                    Some(json!({}))
                )
                .status,
            200
        );
    }
    let id = server.create();
    setup::send(&server, &id, "Spend earliest reset.");
    assert_eq!(server.idle(&id)["meta"]["last_slot"], setup::TWO);
    setup::send(&server, &id, "Keep warm conversation.");
    assert_eq!(server.idle_turns(&id, 2)["meta"]["last_slot"], setup::TWO);
    assert_eq!(
        server
            .call(
                "PUT",
                "/api/settings/defaults",
                Some(json!({"rotation":"round-robin"}))
            )
            .status,
        200
    );
    let next = server.create();
    setup::send(&server, &next, "Least recently picked.");
    assert_eq!(server.idle(&next)["meta"]["last_slot"], setup::ONE);
    let third = server.create();
    setup::send(&server, &third, "Next round robin account.");
    assert_eq!(server.idle(&third)["meta"]["last_slot"], setup::TWO);
    setup::evidence(
        "rotation-order",
        json!({"perishableFirst":setup::TWO,"warmAffinity":setup::TWO,"roundRobin":[setup::ONE,setup::TWO]}),
    );
    server.stop();
}

#[test]
fn rate_limit_with_quota_left_uses_escalating_short_cooldowns() {
    for (strikes, seconds) in [(0, 60), (1, 300), (2, 900)] {
        let mut server = Server::new_stopped(&format!("rate-{strikes}"), true);
        setup::script(&server, json!({"scenarios_by_slot":{setup::ONE:"rate"}}));
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64();
        let routes=(0..strikes).map(|_| json!({"slot":setup::ONE,"bucket":"gemini","event":"cooldown","secs":60,"why":"prior fixture","ts":now-10.0}).to_string()).collect::<Vec<_>>().join("\n");
        setup::write(&server.data.join("state/routing.jsonl"), &(routes + "\n"));
        setup::start(&mut server);
        let id = server.create();
        setup::send(&server, &id, "Rate-limit handoff.");
        let completed = server.idle_turns(&id, 2);
        assert_eq!(completed["meta"]["last_slot"], setup::TWO);
        let accounts = server.call("GET", "/api/accounts", None).json();
        assert_eq!(accounts[0]["quota"]["gemini"]["5h"][0], 0.9);
        assert_eq!(
            accounts[0]["why"]["gemini"],
            format!("rate limited ({}x)", strikes + 1)
        );
        let rows: Vec<Value> = fs::read_to_string(server.data.join("state/routing.jsonl"))
            .unwrap()
            .lines()
            .filter(|s| !s.is_empty())
            .map(|s| serde_json::from_str(s).unwrap())
            .collect();
        let latest = rows.iter().rfind(|row| row["event"] == "cooldown").unwrap();
        assert_eq!(latest["secs"].as_f64(), Some(f64::from(seconds)));
        assert_eq!(latest["why"], "rate limited, quota left");
        assert_eq!(accounts[0]["walls"]["cg"], 0.0);
        server.stop();
    }
    setup::evidence(
        "rate-cooldowns",
        json!({"quotaRemains":0.9,"seconds":[60,300,900],"otherBasketUnwalled":true,"handoffAccount":setup::TWO}),
    );
}
