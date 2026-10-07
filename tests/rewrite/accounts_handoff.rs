//! Proves real HTTP turn rotation without duplicate prompts or stale usage.
use crate::{setup, support::Server};
use serde_json::json;

#[test]
fn unreadable_quota_after_wall_uses_fallback_and_preserves_other_basket() {
    let mut server = Server::new_stopped("quota-unreadable", true);
    setup::script(
        &server,
        json!({"scenarios_by_slot":{setup::ONE:"quota"},"metadata_by_slot":{setup::ONE:{"quota_error":"metadata unavailable"}}}),
    );
    setup::start(&mut server);
    let id = server.create();
    setup::send(&server, &id, "Fallback wall after unreadable quota.");
    let completed = server.idle_turns(&id, 2);
    assert_eq!(completed["meta"]["last_slot"], setup::TWO);
    let rows = server.call("GET", "/api/routing", None).json();
    let wall = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["event"] == "wall" && row["why"] == "quota error, panel unreadable")
        .unwrap();
    // The wall is written after turn_end and before its routing record receives a timestamp.
    let failed_end = completed["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["t"] == "turn_end" && row["turn"] == 1)
        .unwrap()["ts"]
        .as_f64()
        .unwrap();
    let until = wall["until"].as_f64().unwrap();
    assert!(until >= failed_end + 18600.0 && until <= wall["ts"].as_f64().unwrap() + 18600.0);
    let accounts = server.call("GET", "/api/accounts", None).json();
    assert_eq!(accounts[0]["walls"]["cg"], 0.0);
    assert_eq!(accounts[0]["why"]["gemini"], "quota error");
    setup::evidence(
        "quota-fallback",
        json!({"fallbackSeconds":18600,"otherBasketUnwalled":true,"handoffAccount":setup::TWO}),
    );
    server.stop();
}

#[test]
fn quota_wall_hands_off_same_conversation_and_counts_current_steps_once() {
    let mut server = Server::new_stopped("quota-handoff", true);
    setup::script(
        &server,
        json!({"scenarios_by_slot":{setup::ONE:"quota",setup::TWO:"stale_result"},"metadata_by_slot":{setup::ONE:{"quota_after_turn":setup::healthy_quota(0.01)}}}),
    );
    setup::start(&mut server);
    let id = server.create();
    setup::send(&server, &id, "Original handoff request.");
    let completed = server.idle_turns(&id, 2);
    let calls = setup::turn_calls(&server);
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0]["slot"], setup::ONE);
    assert_eq!(calls[1]["slot"], setup::TWO);
    let conversation = completed["meta"]["conversation_id"].as_str().unwrap();
    assert_eq!(
        setup::argument(&calls[1], "--conversation"),
        Some(conversation)
    );
    assert_eq!(
        setup::argument(&calls[0], "-p"),
        Some(
            format!(
                "{}\n\nOriginal handoff request.",
                antiquota_core::jobs::JOBS_HINT
            )
            .as_str()
        )
    );
    assert_ne!(
        setup::argument(&calls[1], "-p"),
        Some(
            format!(
                "{}\n\nOriginal handoff request.",
                antiquota_core::jobs::JOBS_HINT
            )
            .as_str()
        )
    );
    let events = completed["events"].as_array().unwrap();
    assert_eq!(
        events.iter().filter(|event| event["t"] == "user").count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| event["t"] == "turn_end")
            .count(),
        2
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| event["t"] == "notice" && event["kind"] == "handoff")
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .find(|event| event["t"] == "turn_end" && event["turn"] == 2)
            .unwrap()["status"],
        "SUCCESS"
    );
    assert_eq!(completed["meta"]["usage"]["input"], 100);
    assert_eq!(completed["meta"]["usage"]["output"], 10);
    let accounts = server.call("GET", "/api/accounts", None).json();
    assert!(accounts[0]["walls"]["gemini"].as_f64().unwrap() > 0.0);
    assert_eq!(accounts[0]["walls"]["cg"], 0.0);
    assert_eq!(accounts[1]["walls"]["gemini"], 0.0);
    assert_eq!(
        accounts[1]["today"],
        json!({"input":100,"output":10,"thinking":0,"turns":1})
    );
    setup::evidence(
        "quota-handoff",
        json!({"runnerCalls":calls,"userEvents":1,"sameConversation":true,"currentStepInput":100,"currentStepOutput":10,"today":accounts[1]["today"]}),
    );
    server.stop();
    setup::start(&mut server);
    let recovered = server
        .call("GET", &format!("/api/sessions/{id}"), None)
        .json();
    assert_eq!(recovered, completed);
    assert_eq!(
        server.call("GET", "/api/accounts", None).json()[1]["today"],
        accounts[1]["today"]
    );
    server.stop();
}

#[test]
fn quota_without_initialization_retries_original_prompt_without_resume_flag() {
    let mut server = Server::new_stopped("quota-no-init", true);
    setup::script(
        &server,
        json!({"scenarios_by_slot":{setup::ONE:"quota_no_init"},"metadata_by_slot":{setup::ONE:{"quota_after_turn":setup::healthy_quota(0.01)}}}),
    );
    setup::start(&mut server);
    let id = server.create();
    setup::send(&server, &id, "Request before conversation existed.");
    let completed = server.idle_turns(&id, 2);
    let calls = setup::turn_calls(&server);
    assert_eq!(calls.len(), 2);
    for call in &calls {
        assert_eq!(
            setup::argument(call, "-p"),
            Some(
                format!(
                    "{}\n\nRequest before conversation existed.",
                    antiquota_core::jobs::JOBS_HINT
                )
                .as_str()
            )
        );
        assert_eq!(setup::argument(call, "--conversation"), None);
    }
    assert_eq!(completed["meta"]["last_slot"], setup::TWO);
    assert_eq!(
        completed["events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|event| event["t"] == "user")
            .count(),
        1
    );
    setup::evidence(
        "quota-no-init",
        json!({"runnerCalls":2,"originalPromptRetried":true,"resumeFlags":0,"userEvents":1}),
    );
    server.stop();
}

#[test]
fn lost_conversation_recovers_with_summary_and_a_fresh_id() {
    let mut server = Server::new_stopped("conversation-loss", true);
    setup::script(
        &server,
        json!({"scenarios_by_turn":["success","conversation_lost","success"]}),
    );
    setup::start(&mut server);
    let id = server.create();
    setup::send(&server, &id, "Remember this context for recovery.");
    let first = server.idle(&id);
    let original = first["meta"]["conversation_id"].as_str().unwrap();
    setup::send(&server, &id, "Continue after lost conversation.");
    let completed = server.idle_turns(&id, 3);
    let calls = setup::turn_calls(&server);
    assert_eq!(calls.len(), 3);
    assert_eq!(setup::argument(&calls[1], "--conversation"), Some(original));
    assert_eq!(setup::argument(&calls[2], "--conversation"), None);
    let summary = setup::argument(&calls[2], "-p").unwrap();
    assert!(summary.contains("Remember this context for recovery."));
    assert!(summary.contains("Continue after lost conversation."));
    assert_ne!(completed["meta"]["conversation_id"], original);
    assert_eq!(
        completed["events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|event| event["t"] == "user")
            .count(),
        2
    );
    assert_eq!(completed["meta"]["usage"]["input"], 200);
    setup::evidence(
        "conversation-loss",
        json!({"sameIdOnFailedResume":true,"freshIdOnRecovery":true,"summaryRetainsBothPrompts":true,"userEvents":2,"currentInput":200}),
    );
    server.stop();
}
