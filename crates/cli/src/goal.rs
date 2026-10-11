//! Owns `antiquota goal`: a headless goal run that streams its session in the terminal.
//! It uses the running engine (so the session shows up in the app), or starts one for the
//! duration of the run when none is running.
use crate::{engine::engine, follow::Follow};
use anyhow::Context;
use clap::Args;
use serde_json::json;
use std::time::Duration;

#[derive(Args, Debug)]
pub struct GoalArgs {
    /// What the agent works on until it is verified done.
    objective: String,
    #[arg(long, default_value = ".")]
    cwd: String,
    /// A model id from `agy models`; effort is part of it.
    #[arg(long, default_value = antiquota_core::DEFAULT_MODEL)]
    model: String,
    #[arg(long, default_value = "accept-edits", value_parser = ["default", "accept-edits", "plan", "bypass"])]
    mode: String,
    /// Shell command that must exit 0 when the goal is met.
    #[arg(long, default_value = "")]
    check: String,
    /// Skip the independent verifier.
    #[arg(long)]
    no_verifier: bool,
    #[arg(long, default_value = antiquota_core::DEFAULT_MODEL)]
    verifier_model: String,
    #[arg(long, default_value_t = 20)]
    max_rounds: u64,
}

/// Runs the goal and returns the exit code: 0 only when the goal is met.
pub async fn run(args: GoalArgs) -> anyhow::Result<i32> {
    let cwd = std::fs::canonicalize(&args.cwd)
        .with_context(|| format!("not a folder: {}", args.cwd))?
        .to_string_lossy()
        .into_owned();
    let (engine, own) = engine().await?;
    let title: String = args.objective.chars().take(60).collect();
    let session = engine
        .call(
            reqwest::Method::POST,
            "/api/sessions",
            json!({"cwd": cwd, "model": args.model, "mode": args.mode, "title": title}),
        )
        .await?;
    let id = session["id"].as_str().context("no session id")?.to_owned();
    let mut stream = engine.events().await?;
    engine
        .call(
            reqwest::Method::POST,
            &format!("/api/sessions/{id}/goal"),
            json!({"objective": args.objective, "check": args.check, "verifier": !args.no_verifier,
                   "verifier_model": args.verifier_model, "max_rounds": args.max_rounds}),
        )
        .await?;
    let mut follow = Follow::new(&id, &cwd);
    let mut buffer = Vec::new();
    let stopped = loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                let _ = engine.call(reqwest::Method::POST, &format!("/api/sessions/{id}/stop"), json!({})).await;
                break true;
            }
            chunk = stream.chunk() => {
                if let Ok(Some(chunk)) = chunk {
                    buffer.extend_from_slice(&chunk);
                    if follow.frames(&mut std::io::stdout().lock(), &mut std::io::stderr(), &mut buffer) {
                        // A queued snapshot can predate reconnect catch-up; confirm completion.
                        let saved = engine.saved(&id).await?;
                        if follow.catch_up(&mut std::io::stdout().lock(), &saved.meta, &saved.events) {
                            break false;
                        }
                    }
                    continue;
                }
                // The engine ends the stream of a reader that falls behind. Open a new one,
                // then catch up from the saved session so no saved event is missed.
                tokio::time::sleep(Duration::from_millis(200)).await;
                stream = engine.events().await?;
                buffer.clear();
                let saved = engine.saved(&id).await?;
                if follow.catch_up(&mut std::io::stdout().lock(), &saved.meta, &saved.events) {
                    break false;
                }
            }
        }
    };
    if stopped {
        println!("\nstopped; session {id} can be resumed in the app");
    }
    if own {
        let _ = engine
            .call(reqwest::Method::POST, "/api/shutdown", json!({}))
            .await;
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
    Ok(if follow.state == "met" && !stopped {
        0
    } else {
        1
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Engine;
    use antiquota_core::{Event, EventRecord, Goal, Session, StreamEvent};
    use antiquota_server::api::SessionResponse;
    use clap::Parser;
    use std::io::{Read, Write};

    #[derive(Parser)]
    struct Line {
        #[command(flatten)]
        goal: GoalArgs,
    }

    #[tokio::test]
    async fn queued_completion_is_confirmed_against_the_current_http_snapshot() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let engine = Engine {
            base: format!("http://{}", listener.local_addr().unwrap()),
            token: "test-token".into(),
            http: reqwest::Client::new(),
        };
        let server = std::thread::spawn(move || {
            for (state, updated) in [("active", 10.0), ("met", 9.0)] {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = Vec::new();
                let mut bytes = [0; 1024];
                while !request.windows(4).any(|part| part == b"\r\n\r\n") {
                    let count = stream.read(&mut bytes).unwrap();
                    assert!(count > 0);
                    request.extend_from_slice(&bytes[..count]);
                }
                let request = String::from_utf8(request).unwrap();
                assert!(request.starts_with("GET /api/sessions/test HTTP/1.1\r\n"));
                assert!(
                    request
                        .to_lowercase()
                        .contains("x-antiquota-token: test-token\r\n")
                );
                let saved = SessionResponse {
                    meta: Session {
                        id: "test".into(),
                        status: "idle".into(),
                        updated,
                        goal: Some(Goal {
                            state: state.into(),
                            ..Goal::default()
                        }),
                        ..Session::default()
                    },
                    events: vec![],
                };
                let body = serde_json::to_string(&saved).unwrap();
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            }
        });
        let mut follow = Follow::new("test", "/work");
        let queued = StreamEvent {
            sid: Some("test".into()),
            record: EventRecord::new(
                10.0,
                Event::Meta {
                    meta: Box::new(Session {
                        status: "idle".into(),
                        updated: 10.0,
                        goal: Some(Goal {
                            state: "stopped".into(),
                            ..Goal::default()
                        }),
                        ..Session::default()
                    }),
                },
            ),
        };
        let mut out = vec![];
        let mut buffer =
            format!("data: {}\n\n", serde_json::to_string(&queued).unwrap()).into_bytes();
        assert!(follow.frames(&mut out, &mut vec![], &mut buffer));
        let saved = engine.saved("test").await.unwrap();
        assert!(!follow.catch_up(&mut out, &saved.meta, &saved.events));
        assert_eq!(follow.state, "active");
        let saved = engine.saved("test").await.unwrap();
        assert!(follow.catch_up(&mut out, &saved.meta, &saved.events));
        assert_eq!(follow.state, "met");
        server.join().unwrap();
    }

    #[test]
    fn a_goal_defaults_to_gemini_twenty_rounds_and_the_verifier() {
        let goal = Line::try_parse_from(["goal", "Ship it"]).unwrap().goal;
        assert_eq!(goal.objective, "Ship it");
        assert_eq!(goal.cwd, ".");
        assert_eq!(goal.model, "gemini-3.8-flash-high");
        assert_eq!(goal.verifier_model, "gemini-3.8-flash-high");
        assert_eq!(goal.mode, "accept-edits");
        assert_eq!(goal.check, "");
        assert!(!goal.no_verifier);
        assert_eq!(goal.max_rounds, 20);
        let goal = Line::try_parse_from([
            "goal",
            "Fix",
            "--mode",
            "plan",
            "--check",
            "make test",
            "--no-verifier",
            "--max-rounds",
            "3",
        ])
        .unwrap()
        .goal;
        assert_eq!(
            (goal.mode.as_str(), goal.check.as_str()),
            ("plan", "make test")
        );
        assert!(goal.no_verifier);
        assert_eq!(goal.max_rounds, 3);
        assert!(Line::try_parse_from(["goal", "x", "--mode", "yolo"]).is_err());
        assert!(Line::try_parse_from(["goal"]).is_err());
    }
}
