//! Owns local engine discovery and the authenticated HTTP client shared by terminal commands.
use antiquota_server::api::SessionResponse;
use anyhow::{Context, bail};
use serde_json::{Value, json};
use std::time::Duration;

pub(crate) struct Engine {
    pub(crate) base: String,
    pub(crate) token: String,
    pub(crate) http: reqwest::Client,
}
impl Engine {
    pub(crate) async fn call(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Value,
    ) -> anyhow::Result<Value> {
        let response = self
            .http
            .request(method, format!("{}{path}", self.base))
            .header("X-Antiquota-Token", &self.token)
            .timeout(Duration::from_secs(15))
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

    pub(crate) async fn events(&self) -> anyhow::Result<reqwest::Response> {
        self.http
            .get(format!("{}/api/events?token={}", self.base, self.token))
            .send()
            .await
            .context("the event stream did not open")?
            .error_for_status()
            .context("the engine refused the event stream")
    }

    pub(crate) async fn saved(&self, id: &str) -> anyhow::Result<SessionResponse> {
        serde_json::from_value(
            self.call(
                reqwest::Method::GET,
                &format!("/api/sessions/{id}"),
                json!({}),
            )
            .await?,
        )
        .context("the saved session is unreadable")
    }
}

/// The running engine, or one started in this process (returned as `true`).
pub(crate) async fn engine() -> anyhow::Result<(Engine, bool)> {
    connect(false).await
}

/// Interactive sessions share a background engine that survives terminal exit.
pub(crate) async fn interactive_engine() -> anyhow::Result<Engine> {
    Ok(connect(true).await?.0)
}

async fn connect(background: bool) -> anyhow::Result<(Engine, bool)> {
    let mut own = false;
    let mut child = None;
    if antiquota_server::discover().await.is_err() {
        if background {
            use std::process::{Command, Stdio};
            let mut command = Command::new(std::env::current_exe()?);
            command
                .args(["serve", "--port", "0", "--no-open"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            // A closed terminal must not send its foreground process group's hangup to the engine.
            #[cfg(unix)]
            {
                use std::os::unix::process::CommandExt;
                command.process_group(0);
            }
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                command.creation_flags(0x0000_0008 | 0x0000_0200);
            }
            child = Some(
                command
                    .spawn()
                    .context("the background engine did not start")?,
            );
        } else {
            own = true;
            tokio::spawn(async {
                if let Err(error) = antiquota_server::serve(0, false).await {
                    eprintln!("antiquota: {error}");
                }
            });
        }
    }
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    let info = loop {
        if let Ok(info) = antiquota_server::discover().await {
            break info;
        }
        if let Some(child) = child.as_mut()
            && child.try_wait()?.is_some()
        {
            bail!("antiquota: the background engine exited before discovery");
        }
        if tokio::time::Instant::now() > deadline {
            if let Some(child) = child.as_mut() {
                let _ = child.kill();
                let _ = child.wait();
            }
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
        http: reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .build()?,
    };
    Ok((engine, own && info.pid == std::process::id()))
}
