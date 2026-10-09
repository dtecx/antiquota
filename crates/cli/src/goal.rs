//! Owns `antiquota goal`: a headless goal run that streams its session in the terminal.
//! It uses the running engine (so the session shows up in the app), or starts one for the
//! duration of the run when none is running.
use crate::follow::Follow;
use antiquota_server::api::SessionResponse;
use anyhow::{Context, bail};
use clap::Args;
use serde_json::{Value, json};
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

struct Engine {
    base: String,
    token: String,
    http: reqwest::Client,
}
impl Engine {
    async fn call(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Value,
    ) -> anyhow::Result<Value> {
        let response = self
            .http
            .request(method, format!("{}{path}", self.base))
            .header("X-Antiquota-Token", &self.token)
            .json(&body)
            .send()
            .await
            .context("the engine did not answer")?;
        let status = response.status();
        let value: Value = response.json().await.unwrap_or(Value::Null);
        if !status.is_success() {
            bail!(
                "{}",
                value["error"]
                    .as_str()
                    .unwrap_or("the engine refused the request")
            );
        }
        Ok(value)
    }

    async fn events(&self) -> anyhow::Result<reqwest::Response> {
        self.http
            .get(format!("{}/api/events?token={}", self.base, self.token))
            .send()
            .await
            .context("the event stream did not open")
    }
}

/// The running engine, or one started in this process (returned as `true`).
async fn engine() -> anyhow::Result<(Engine, bool)> {
    let mut own = false;
    if antiquota_server::discover().await.is_err() {
        own = true;
        tokio::spawn(async {
            if let Err(error) = antiquota_server::serve(0, false).await {
                eprintln!("antiquota: {error}");
            }
        });
    }
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    let info = loop {
        if let Ok(info) = antiquota_server::discover().await {
            break info;
        }
        if tokio::time::Instant::now() > deadline {
            bail!("antiquota: the engine did not start");
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    let root = antiquota_store::data_root()?;
    let token = std::fs::read_to_string(root.join("state/gui-token"))
        .context("the engine token is unreadable")?
        .trim()
        .to_owned();
    let engine = Engine {
        base: format!("http://127.0.0.1:{}", info.port),
        token,
        http: reqwest::Client::new(),
    };
    Ok((engine, own))
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
                        break false;
                    }
                    continue;
                }
                // The engine ends the stream of a reader that falls behind. Open a new one,
                // then catch up from the saved session so no saved event is missed.
                tokio::time::sleep(Duration::from_millis(200)).await;
                stream = engine.events().await?;
                buffer.clear();
                let saved: SessionResponse = serde_json::from_value(
                    engine.call(reqwest::Method::GET, &format!("/api/sessions/{id}"), json!({})).await?,
                )
                .context("the saved session is unreadable")?;
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
    use super::GoalArgs;
    use clap::Parser;

    #[derive(Parser)]
    struct Line {
        #[command(flatten)]
        goal: GoalArgs,
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
