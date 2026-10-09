//! Owns agy's stream-json normalization and completed-step usage accounting.
use antiquota_core::{Event, MAX_SAFE_INTEGER, PortError, StepUsage, TurnResult};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
pub struct Decoder {
    turn: u64,
    conversation: Option<String>,
    texts: BTreeMap<u64, String>,
    done: BTreeSet<u64>,
    usage_done: BTreeSet<u64>,
    usage: StepUsage,
    last: Option<(String, String)>,
    result: Value,
    errors: Vec<String>,
}
impl Decoder {
    pub fn new(turn: u64) -> Self {
        Self {
            turn,
            conversation: None,
            texts: BTreeMap::new(),
            done: BTreeSet::new(),
            usage_done: BTreeSet::new(),
            usage: StepUsage::default(),
            last: None,
            result: json!({}),
            errors: vec![],
        }
    }
    pub fn conversation_id(&self) -> Option<&str> {
        self.conversation.as_deref()
    }
    pub fn accept(&mut self, value: Value) -> Result<Vec<Event>, PortError> {
        if let Some(conv) = value["conversation_id"].as_str() {
            self.conversation = Some(conv.into());
        }
        if value["event"] != "result" && antiquota_core::error_frame(&value) {
            // agy's own error frame; the model's reply never counts as an error channel.
            let mut frame = value.clone();
            if let Some(object) = frame.as_object_mut() {
                object.remove("response");
            }
            self.errors.push(frame.to_string());
            return Ok(vec![]);
        }
        if value["event"] == "result" {
            self.result = value["result"].clone();
            if let Some(conv) = self.result["conversation_id"].as_str() {
                self.conversation = Some(conv.into());
            }
            return Ok(vec![]);
        }
        if value["event"] != "step_update" {
            return Ok(vec![]);
        }
        let step = &value["step_update"];
        let kind = step["step_type"].as_str().unwrap_or("");
        let state = step["state"].as_str().unwrap_or("");
        self.last = Some((kind.into(), state.into()));
        let index = step["step_index"].as_u64().unwrap_or(0);
        let mut events = vec![];
        if state == "DONE" && step["usage"].is_object() && self.usage_done.insert(index) {
            let count = |key: &str| step["usage"][key].as_u64().unwrap_or(0);
            for (target, key) in [
                (&mut self.usage.input_tokens, "input_tokens"),
                (&mut self.usage.output_tokens, "output_tokens"),
                (&mut self.usage.thinking_tokens, "thinking_tokens"),
                (&mut self.usage.total_tokens, "total_tokens"),
                (&mut self.usage.cache_read_tokens, "cache_read_tokens"),
            ] {
                *target = target
                    .checked_add(count(key))
                    .filter(|x| *x <= MAX_SAFE_INTEGER)
                    .ok_or_else(|| {
                        PortError::Invalid("agy usage exceeds safe integer range".into())
                    })?;
            }
            self.usage.context = self
                .usage
                .context
                .max(count("input_tokens").saturating_add(count("cache_read_tokens")));
        }
        if kind == "agent_response" {
            let delta = step["text_delta"].as_str().unwrap_or("");
            let text = self.texts.entry(index).or_default();
            if text.len() + delta.len() > 4_000_000 {
                return Err(PortError::Invalid("agy response exceeds 4 MB".into()));
            }
            if !delta.is_empty() && !self.done.contains(&index) {
                text.push_str(delta);
                events.push(Event::Delta {
                    turn: self.turn,
                    step: index,
                    text: delta.into(),
                });
            }
            if state == "DONE" && self.done.insert(index) {
                events.push(Event::Assistant {
                    turn: self.turn,
                    step: index,
                    text: text.clone(),
                });
            }
        } else if kind == "tool" {
            let info = &step["tool_info"];
            let state = match state {
                "ACTIVE" => "running",
                "DONE" => "done",
                _ => "error",
            };
            let output = info["output"]
                .as_str()
                .unwrap_or("")
                .chars()
                .take(20_000)
                .collect();
            events.push(Event::Tool {
                turn: self.turn,
                step: index,
                name: info["name"]
                    .as_str()
                    .or_else(|| step["tool_name"].as_str())
                    .unwrap_or("tool")
                    .into(),
                params: info.get("parameters").cloned().unwrap_or(json!({})),
                state: state.into(),
                secs: step["duration_seconds"].as_f64(),
                output: (state != "running").then_some(output),
                diff: None,
                error: info.get("error").cloned(),
            });
        }
        Ok(events)
    }
    pub fn finish(self, code: i32, stopped: bool, stderr: &str) -> TurnResult {
        let replied = self
            .last
            .as_ref()
            .is_some_and(|(kind, state)| kind == "agent_response" && state == "DONE");
        let genuine = self
            .errors
            .iter()
            .map(String::as_str)
            .chain(self.result["error"].as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let genuine_text = format!("{}{}", genuine, if code != 0 { stderr } else { "" });
        let status = if stopped {
            "stopped".into()
        } else if replied || code == 0 && self.result["status"] == "SUCCESS" {
            "SUCCESS".into()
        } else if let Some(failure) = antiquota_core::classify_error(&genuine_text) {
            match failure {
                antiquota_core::Failure::Quota => "quota",
                antiquota_core::Failure::Auth => "auth",
                antiquota_core::Failure::Transient => "transient",
            }
            .into()
        } else {
            self.result["status"].as_str().unwrap_or("ERROR").into()
        };
        let error = if status == "SUCCESS" || stopped {
            String::new()
        } else {
            format!("{}{}", genuine, stderr)
                .chars()
                .take(4000)
                .collect()
        };
        TurnResult {
            status,
            response: self.result["response"].as_str().unwrap_or_default().into(),
            conversation_id: self.conversation,
            usage: self.usage,
            error,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duplicate_done_and_stale_result_do_not_double_count_or_fail_success() {
        let mut d = Decoder::new(1);
        d.accept(json!({"event":"step_update","step_update":{"step_index":1,"step_type":"agent_response","state":"ACTIVE","text_delta":"ok"}})).unwrap();
        let done = json!({"event":"step_update","step_update":{"step_index":1,"step_type":"agent_response","state":"DONE","usage":{"input_tokens":100,"output_tokens":10,"total_tokens":110}}});
        d.accept(done.clone()).unwrap();
        assert!(d.accept(done).unwrap().is_empty());
        d.accept(json!({"event":"result","result":{"status":"ERROR","error":"Individual quota reached","usage":{"input_tokens":999999}}})).unwrap();
        let result = d.finish(0, false, "");
        assert_eq!(result.status, "SUCCESS");
        assert_eq!(result.usage.input_tokens, 100);
        assert_eq!(result.usage.output_tokens, 10);
    }
    #[test]
    fn cached_input_and_thinking_add_up_over_completed_steps() {
        let mut d = Decoder::new(1);
        for (step, cache) in [(1, 300), (2, 500)] {
            d.accept(json!({"event":"step_update","step_update":{"step_index":step,"step_type":"agent_response","state":"DONE","usage":{"input_tokens":20,"output_tokens":5,"thinking_tokens":7,"cache_read_tokens":cache}}})).unwrap();
        }
        let usage = d.finish(0, false, "").usage;
        assert_eq!(
            (
                usage.input_tokens,
                usage.cache_read_tokens,
                usage.thinking_tokens
            ),
            (40, 800, 14)
        );
        // The context is the largest prompt one step sent, cached part included.
        assert_eq!(usage.context, 520);
        let wire = serde_json::to_value(&usage).unwrap();
        assert_eq!(wire["cache_read_tokens"], 800);
        assert!(
            serde_json::to_value(StepUsage::default())
                .unwrap()
                .get("cache_read_tokens")
                .is_none()
        );
    }
    #[test]
    fn only_agy_error_channels_classify_a_failed_turn() {
        let words = "RESOURCE_EXHAUSTED quota 429; UNAUTHENTICATED token expired; 503 UNAVAILABLE";
        // A successful reply may discuss errors, and successful stderr chatter is ignored.
        let mut d = Decoder::new(1);
        d.accept(json!({"event":"step_update","step_update":{"step_index":1,"step_type":"agent_response","state":"DONE","text_delta":words}})).unwrap();
        d.accept(json!({"event":"result","result":{"status":"SUCCESS","response":words}}))
            .unwrap();
        let result = d.finish(
            0,
            false,
            "warning: rate limit quota 503 UNAVAILABLE chatter",
        );
        assert_eq!(
            (result.status.as_str(), result.error.as_str()),
            ("SUCCESS", "")
        );
        // A failed result's response text is not an error channel.
        let failed = |error: &str, code: i32, stderr: &str| {
            let mut d = Decoder::new(1);
            d.accept(json!({"event":"result","result":{"status":"ERROR","error":error,"response":words}}))
                .unwrap();
            d.finish(code, false, stderr).status
        };
        assert_eq!(failed("tool crashed", 1, ""), "ERROR");
        // stderr counts only when agy exits with an error.
        assert_eq!(failed("tool crashed", 0, "UNAUTHENTICATED"), "ERROR");
        assert_eq!(failed("tool crashed", 1, "UNAUTHENTICATED"), "auth");
        assert_eq!(failed("Individual quota reached", 1, ""), "quota");
        // agy's own error frame counts; its response field does not.
        let mut d = Decoder::new(1);
        d.accept(
            json!({"type":"error","message":"UNAVAILABLE: overloaded","response":"quota 429"}),
        )
        .unwrap();
        let result = d.finish(1, false, "");
        assert_eq!(result.status, "transient");
        assert!(!result.error.contains("429"));
    }
    #[test]
    fn crash_and_stopping_are_distinct() {
        assert_eq!(Decoder::new(1).finish(17, false, "crash").status, "ERROR");
        assert_eq!(Decoder::new(1).finish(-1, true, "").status, "stopped");
    }
}
