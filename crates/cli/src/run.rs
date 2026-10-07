//! Owns a CLI turn through the shared HTTP server and typed SSE stream.
use antiquota_core::{Session, StreamEvent};
use antiquota_server::api::SendResponse;
use antiquota_store::EngineInfo;
use std::io::{BufRead, BufReader};
pub fn run(
    info: EngineInfo,
    cwd: Option<String>,
    model: Option<String>,
    slot: Option<String>,
    text: String,
) -> anyhow::Result<()> {
    if slot.is_some() {
        anyhow::bail!("explicit account selection is pending the routing slice");
    }
    let token = std::fs::read_to_string(antiquota_store::data_root()?.join("state/gui-token"))?;
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .build()?;
    let base = info.url.trim_end_matches('/');
    let cwd = cwd.unwrap_or(std::env::current_dir()?.to_string_lossy().into_owned());
    let session:Session=client.post(format!("{base}/api/sessions")).header("X-Antiquota-Token",token.trim()).json(&serde_json::json!({"cwd":cwd,"model":model.unwrap_or_else(||antiquota_core::DEFAULT_MODEL.into())})).send()?.error_for_status()?.json()?;
    let stream = client
        .get(format!("{base}/api/events?token={}", token.trim()))
        .send()?
        .error_for_status()?;
    let _: SendResponse = client
        .post(format!("{base}/api/sessions/{}/send", session.id))
        .header("X-Antiquota-Token", token.trim())
        .json(&serde_json::json!({"text":text}))
        .send()?
        .error_for_status()?
        .json()?;
    let mut waiting = false;
    let mut error = None;
    for line in BufReader::new(stream).lines() {
        let line = line?;
        let Some(data) = line.strip_prefix("data:") else {
            continue;
        };
        let event: StreamEvent = serde_json::from_str(data.trim())?;
        if event.sid.as_deref() != Some(&session.id) {
            continue;
        }
        match event.record.event {
            antiquota_core::Event::Delta { text, .. } => {
                use std::io::Write;
                print!("{text}");
                std::io::stdout().flush()?;
            }
            antiquota_core::Event::TurnEnd { status, .. } => {
                println!();
                if status != "SUCCESS" {
                    anyhow::bail!("turn ended with {status}");
                }
                return Ok(());
            }
            antiquota_core::Event::Notice { kind, text, .. } if kind == "error" => {
                eprintln!("{text}");
                error = Some(text);
            }
            antiquota_core::Event::Meta { meta } => {
                if meta.status != "idle" {
                    waiting = true;
                } else if waiting {
                    anyhow::bail!(
                        "{}",
                        error.unwrap_or_else(|| "turn stopped before completion".into())
                    );
                }
            }
            _ => {}
        }
    }
    anyhow::bail!("event stream closed before the turn completed")
}
