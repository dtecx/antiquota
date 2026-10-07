//! Proves cancellation, crashes, and protocol errors cannot retain descendant pipes.
mod support;
use antiquota_agy::AgyRunner;
use antiquota_core::{
    AgentRunner, Cancellation, Event, Mode, PortError, Slot, TurnRequest, TurnSink,
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
#[derive(Default)]
struct Capture {
    events: Vec<Event>,
    conversation: Option<String>,
}
struct Sink(Arc<Mutex<Capture>>);
impl TurnSink for Sink {
    fn event(&mut self, event: Event) -> Result<(), PortError> {
        let mut capture = self.0.lock().unwrap();
        assert_eq!(capture.conversation.as_deref(), Some("early-resume-id"));
        capture.events.push(event);
        Ok(())
    }
    fn conversation_id(&mut self, id: &str) -> Result<(), PortError> {
        self.0.lock().unwrap().conversation = Some(id.into());
        Ok(())
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn inherited_pipes_close_on_stop_crash_and_protocol_failure() {
    for scenario in [
        "cancel",
        "crash",
        "protocol-error",
        "success-with-descendant",
        "drop",
    ] {
        let sandbox = support::Sandbox::new(scenario);
        let runner = AgyRunner {
            executable: sandbox.executable.clone(),
            hook_program: "not-used".into(),
            isolated_fake: true,
        };
        let request = TurnRequest {
            session: "abcdef123456".into(),
            turn: 1,
            prompt: scenario.into(),
            model: "synthetic".into(),
            mode: Mode::Default,
            conversation_id: None,
            cwd: sandbox.root.to_string_lossy().into_owned(),
            slot: Slot {
                name: "one@example.com".into(),
                home: sandbox.token_home().to_string_lossy().into_owned(),
            },
            state_dir: sandbox.root.join("state").to_string_lossy().into_owned(),
            add_dirs: vec![],
        };
        let cancellation = Cancellation::default();
        let signal = cancellation.clone();
        let capture = Arc::new(Mutex::new(Capture::default()));
        let sink = Sink(capture.clone());
        let running =
            tokio::spawn(async move { runner.run(request, signal, Box::new(sink)).await });
        sandbox.wait_for("descendant-ready");
        let started = std::time::Instant::now();
        if scenario == "cancel" {
            cancellation.cancel();
        } else if scenario == "drop" {
            running.abort();
        }
        let completion = tokio::time::timeout(Duration::from_secs(3), running)
            .await
            .unwrap();
        assert!(started.elapsed() < Duration::from_secs(3));
        if scenario == "drop" {
            assert!(completion.unwrap_err().is_cancelled());
        } else {
            let result = completion.unwrap();
            match scenario {
                "cancel" => assert_eq!(result.unwrap().status, "stopped"),
                "crash" => assert_eq!(result.unwrap().status, "ERROR"),
                "success-with-descendant" => assert_eq!(result.unwrap().status, "SUCCESS"),
                _ => assert!(
                    result
                        .unwrap_err()
                        .to_string()
                        .contains("protocol line exceeds")
                ),
            }
        }
        assert_eq!(
            capture.lock().unwrap().conversation.as_deref(),
            Some("early-resume-id")
        );
        tokio::time::sleep(Duration::from_millis(2100)).await;
        assert!(
            !sandbox.root.join("descendant-wrote").exists(),
            "{scenario}: descendant survived termination"
        );
    }
}
