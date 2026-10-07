//! Produce the actual agy init, step_update, and result envelopes.
use crate::{Action, Scenario, Script, TurnContext};
use serde_json::{Value, json};

fn action(event: Value, delay_ms: u64) -> Action {
    Action { event, delay_ms }
}

pub fn frames(script: &Script, ctx: &TurnContext) -> (Vec<Action>, i32) {
    if !script.actions.is_empty() {
        return (script.actions.clone(), script.exit_code.unwrap_or(0));
    }
    let scenario = script.selected(ctx);
    let mut result = vec![action(
        json!({"event":"init","conversation_id":ctx.conversation,
        "init":{"model":"stub","cwd":ctx.cwd}}),
        0,
    )];
    if scenario == Scenario::Crash {
        return (result, 23);
    }
    let error = match scenario {
        Scenario::Quota => Some("RESOURCE_EXHAUSTED: Individual quota reached"),
        Scenario::Rate => Some("RESOURCE_EXHAUSTED: rate limited; retry later"),
        Scenario::Auth => Some("UNAUTHENTICATED: token expired"),
        _ => None,
    };
    let tool_name = if matches!(scenario, Scenario::Approval | Scenario::Hook) {
        "run_command"
    } else {
        "write_to_file"
    };
    let params = if tool_name == "run_command" {
        json!({"CommandLine":"touch fake-hook-marker"})
    } else {
        json!({"TargetFile":format!("{}/notes.txt", ctx.cwd)})
    };
    let tool = json!({"conversation_id":ctx.conversation,"step_index":3,"step_type":"tool",
        "tool_name":tool_name,"tool_info":{"name":tool_name,"parameters":params}});
    let mut active = tool.clone();
    active["state"] = json!("ACTIVE");
    result.push(action(
        json!({"event":"step_update","step_update":active}),
        0,
    ));
    if let Some(error) = error {
        if scenario == Scenario::Quota {
            let mut done = tool;
            done["state"] = json!("DONE");
            done["duration_seconds"] = json!(0.01);
            result.push(action(json!({"event":"step_update","step_update":done}), 0));
            result.push(action(
                json!({"event":"step_update","step_update":{
                "conversation_id":ctx.conversation,"step_index":4,"step_type":"agent_response",
                "state":"ACTIVE","text_delta":"Partial answer before quota."}}),
                0,
            ));
        }
        result.push(action(
            json!({"event":"result","result":{"conversation_id":ctx.conversation,
            "status":"ERROR","error":error}}),
            0,
        ));
        return (result, script.exit_code.unwrap_or(1));
    }
    let delay = if scenario == Scenario::Delay && script.delay_ms == 0 {
        10_000
    } else if script.delay_ms == 0 {
        200
    } else {
        script.delay_ms
    };
    let mut done = tool;
    done["state"] = json!("DONE");
    done["duration_seconds"] = json!(0.01);
    result.push(action(
        json!({"event":"step_update","step_update":done}),
        delay,
    ));
    let mut reply = script
        .reply
        .clone()
        .unwrap_or_else(|| format!("turn {} on {}", ctx.turn, ctx.slot));
    if scenario == Scenario::Huge {
        reply = "x".repeat(script.huge_bytes.min(8_000_000));
    }
    if scenario == Scenario::Background {
        reply.push_str(&format!(
            "\nRUN_IN_BACKGROUND: `{}`\nCHECK_EVERY: 1m",
            script.background_command
        ));
    }
    if ctx.turn >= script.done_after || scenario == Scenario::GoalComplete {
        reply.push_str("\nGOAL_COMPLETE");
    }
    let split = reply.char_indices().nth(5).map_or(reply.len(), |(i, _)| i);
    for part in [&reply[..split], &reply[split..]] {
        result.push(action(
            json!({"event":"step_update","step_update":{"conversation_id":ctx.conversation,
            "step_index":4,"step_type":"agent_response","state":"ACTIVE","text_delta":part}}),
            0,
        ));
    }
    let complete = json!({"event":"step_update","step_update":{"conversation_id":ctx.conversation,
        "step_index":4,"step_type":"agent_response","state":"DONE","text_delta":"",
        "usage":{"input_tokens":script.input_tokens,"output_tokens":script.output_tokens,"total_tokens":script.input_tokens+script.output_tokens}}});
    result.push(action(complete.clone(), 0));
    if scenario == Scenario::DuplicateDone {
        result.push(action(complete, 0));
    }
    result.push(action(json!({"event":"result","result":{"conversation_id":ctx.conversation,
        "status":if scenario==Scenario::StaleResult {"ERROR"} else {"SUCCESS"},"response":reply,
        "error":if scenario==Scenario::StaleResult {json!("Individual quota reached. Resets in 110h50m31s.")} else {Value::Null},
        "usage":{"input_tokens":script.result_input_tokens,"output_tokens":script.result_output_tokens,"total_tokens":script.result_input_tokens+script.result_output_tokens}}}), 0));
    (result, script.exit_code.unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ctx() -> TurnContext {
        TurnContext {
            slot: "one@example.com".into(),
            cwd: "/fixture".into(),
            conversation: "existing-conversation".into(),
            turn: 1,
        }
    }
    #[test]
    fn every_scenario_uses_real_envelopes_and_retains_resume_identity() {
        for scenario in [
            Scenario::Success,
            Scenario::Delay,
            Scenario::Quota,
            Scenario::Rate,
            Scenario::Auth,
            Scenario::Approval,
            Scenario::Hook,
            Scenario::Background,
            Scenario::GoalComplete,
            Scenario::Huge,
            Scenario::Crash,
            Scenario::StaleResult,
            Scenario::DuplicateDone,
        ] {
            let (frames, code) = frames(
                &Script {
                    scenario,
                    huge_bytes: 100,
                    ..Default::default()
                },
                &ctx(),
            );
            assert_eq!(frames[0].event["event"], "init");
            assert_eq!(frames[0].event["conversation_id"], "existing-conversation");
            assert!(frames.iter().all(|a| {
                ["init", "step_update", "result"].contains(&a.event["event"].as_str().unwrap())
            }));
            assert_eq!(
                code,
                if scenario == Scenario::Crash {
                    23
                } else if matches!(scenario, Scenario::Quota | Scenario::Rate | Scenario::Auth) {
                    1
                } else {
                    0
                }
            );
        }
    }
    #[test]
    fn duplicate_done_and_stale_result_expose_usage_deduplication() {
        let (duplicate, _) = frames(
            &Script {
                scenario: Scenario::DuplicateDone,
                ..Default::default()
            },
            &ctx(),
        );
        let completed: Vec<_> = duplicate
            .iter()
            .filter(|a| {
                a.event["step_update"]["step_type"] == "agent_response"
                    && a.event["step_update"]["state"] == "DONE"
            })
            .collect();
        assert_eq!(completed.len(), 2);
        assert_eq!(completed[0].event, completed[1].event);
        let (stale, _) = frames(
            &Script {
                scenario: Scenario::StaleResult,
                ..Default::default()
            },
            &ctx(),
        );
        assert_eq!(stale.last().unwrap().event["result"]["status"], "ERROR");
        assert_eq!(
            stale.last().unwrap().event["result"]["usage"]["input_tokens"],
            9999
        );
        assert!(
            stale
                .iter()
                .any(|a| a.event["step_update"]["usage"]["input_tokens"] == 100)
        );
    }
}
