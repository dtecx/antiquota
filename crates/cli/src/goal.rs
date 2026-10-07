//! Owns `antiquota goal`: a headless goal run that streams its session in the terminal.
//! It uses the running engine (so the session shows up in the app), or starts one for the
//! duration of the run when none is running.
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

fn print_event(event: &Value, cwd: &str) {
    use std::io::Write;
    let mut out = std::io::stdout().lock();
    let text = |key: &str| event[key].as_str().unwrap_or("").to_owned();
    let _ = match event["t"].as_str().unwrap_or("") {
        "delta" => write!(out, "{}", text("text")),
        "assistant" => writeln!(out),
        "user" if event["kind"] != "user" => {
            let body: String = text("text").chars().take(400).collect();
            writeln!(out, "\n>> {}: {body}", text("kind"))
        }
        "turn_start" => writeln!(
            out,
            "\n-- turn {} on {} ({})",
            event["turn"],
            text("slot"),
            text("model")
        ),
        "tool" if event["state"] != "running" => {
            let p = &event["params"];
            let arg = ["CommandLine", "TargetFile", "AbsolutePath", "Query"]
                .iter()
                .find_map(|k| p[k].as_str())
                .unwrap_or("")
                .replace(&format!("{cwd}/"), "");
            let mark = if event["state"] == "error" { 'x' } else { '.' };
            let line: String = format!("   {mark} {} {arg}", text("name"))
                .chars()
                .take(200)
                .collect();
            writeln!(out, "{line}")
        }
        "turn_end" => writeln!(
            out,
            "   {} in {:.1}s",
            text("status"),
            event["secs"].as_f64().unwrap_or(0.0)
        ),
        "notice" => writeln!(out, "== {}: {}", text("kind"), text("text")),
        _ => Ok(()),
    };
    let _ = out.flush();
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
    let mut stream = engine
        .http
        .get(format!("{}/api/events?token={}", engine.base, engine.token))
        .send()
        .await
        .context("the event stream did not open")?;
    engine
        .call(
            reqwest::Method::POST,
            &format!("/api/sessions/{id}/goal"),
            json!({"objective": args.objective, "check": args.check, "verifier": !args.no_verifier,
                   "verifier_model": args.verifier_model, "max_rounds": args.max_rounds}),
        )
        .await?;
    let mut buffer = String::new();
    let mut started = false;
    let mut state = String::from("active");
    let stopped = loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                let _ = engine.call(reqwest::Method::POST, &format!("/api/sessions/{id}/stop"), json!({})).await;
                break true;
            }
            chunk = stream.chunk() => {
                let Some(chunk) = chunk.context("the event stream failed")? else { break false };
                buffer.push_str(&String::from_utf8_lossy(&chunk));
                let mut done = false;
                while let Some(end) = buffer.find("\n\n") {
                    let message: String = buffer.drain(..end + 2).collect();
                    let Some(data) = message.lines().find_map(|l| l.strip_prefix("data: ")) else { continue };
                    let Ok(event) = serde_json::from_str::<Value>(data) else { continue };
                    if event["sid"] != id.as_str() {
                        continue;
                    }
                    if event["t"] == "meta" {
                        let meta = &event["meta"];
                        state = meta["goal"]["state"].as_str().unwrap_or("").to_owned();
                        if meta["status"] != "idle" {
                            started = true;
                        } else if started {
                            done = true;
                        }
                    }
                    print_event(&event, &cwd);
                }
                if done {
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
    Ok(if state == "met" && !stopped { 0 } else { 1 })
}
