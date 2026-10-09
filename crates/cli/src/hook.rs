//! Owns agy's per-call hook: it asks the shared engine, or decides locally when no engine
//! answers. It starts without logging or an async runtime, so each tool call stays fast.
//!
//! `pre-tool` (PreToolUse) decides a tool call; calls that need the user wait in the GUI while
//! agy waits for the answer. `pre-invocation` (PreInvocation) hands agy the messages the user
//! sent mid-turn before its next model call. Calls from turns antiquota did not start are left
//! to agy. Anything that goes wrong refuses the tool call rather than allowing it.
use antiquota_core::{Mode, Permissions, Verdict, policy};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    time::Duration,
};

/// Seconds per long poll while the user decides.
const WAIT: u64 = 25;
const INPUT_LIMIT: u64 = 1_000_000;
const NO_APPROVER: &str = "This needs the user's approval, and antiquota's app is not running to ask them. Continue without it, or explain what you need.";
const LOST: &str =
    "antiquota lost this approval request (its engine restarted). Ask again if you still need it.";
const UNCHECKED: &str = "antiquota could not check this tool call.";

pub fn run(kind: &str) -> anyhow::Result<()> {
    let mut bytes = vec![];
    let read = std::io::stdin()
        .take(INPUT_LIMIT + 1)
        .read_to_end(&mut bytes);
    let oversized = bytes.len() as u64 > INPUT_LIMIT;
    let payload = if read.is_err() || oversized {
        json!({})
    } else {
        serde_json::from_slice(&bytes).unwrap_or_else(|_| json!({}))
    };
    let out = answer(kind, &payload, oversized);
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer(&mut stdout, &out)?;
    stdout.write_all(b"\n")?;
    stdout.flush()?;
    Ok(())
}

fn answer(kind: &str, payload: &Value, oversized: bool) -> Value {
    let pre_tool = kind != "pre-invocation";
    let session = std::env::var("ANTIQUOTA_SESSION").unwrap_or_default();
    if session.is_empty() {
        return if pre_tool {
            json!({"decision": "ask"})
        } else {
            json!({})
        };
    }
    let state = PathBuf::from(std::env::var_os("ANTIQUOTA_STATE").unwrap_or_default());
    if !pre_tool {
        return steer(&state, &session).unwrap_or_else(|error| {
            eprintln!("antiquota hook: {error}");
            json!({})
        });
    }
    if oversized {
        return deny("Hook input exceeds the limit.");
    }
    let (tool, args) = tool_call(payload);
    match ask_engine(&state, &session, &tool, &args) {
        Ok(reply) => reply,
        Err(error) => {
            eprintln!("antiquota hook: {error}");
            let mode = std::env::var("ANTIQUOTA_MODE")
                .ok()
                .and_then(|m| serde_json::from_value::<Mode>(json!(m)).ok())
                .unwrap_or_default();
            decide_here(&state, mode, &tool, &args)
        }
    }
}

/// The tool call in agy's hook payload (`toolCall` with `args`, or `parameters` as in its
/// stream), or the generic `tool_name`/`tool_input` shape.
fn tool_call(payload: &Value) -> (String, Value) {
    let call = &payload["toolCall"];
    let name = call["name"]
        .as_str()
        .or_else(|| payload["tool_name"].as_str())
        .unwrap_or("");
    let args = [&call["args"], &call["parameters"], &payload["tool_input"]]
        .into_iter()
        .find(|v| v.is_object())
        .cloned()
        .unwrap_or_else(|| json!({}));
    (name.into(), args)
}

fn deny(reason: &str) -> Value {
    json!({"decision": "deny", "reason": reason})
}

fn reply(value: &Value) -> Result<Value, String> {
    match value["decision"].as_str() {
        Some("allow") => Ok(json!({"decision": "allow"})),
        Some("deny") => Ok(deny(value["reason"].as_str().unwrap_or(UNCHECKED))),
        // The engine no longer knows this session or request: refuse rather than let a
        // skip-permissions run go ahead unchecked.
        Some("ask") => Ok(deny(LOST)),
        _ => Err("engine reply has no decision".into()),
    }
}

fn ask_engine(state: &Path, session: &str, tool: &str, args: &Value) -> Result<Value, String> {
    let engine = Engine::find(state)?;
    let mut value = engine.call(
        "POST",
        "/api/approvals",
        Some(&json!({"session": session, "tool": tool, "args": args})),
        Duration::from_secs(WAIT + 10),
    )?;
    while let Some(id) = value["pending"].as_str() {
        let path = format!("/api/approvals/{id}?wait={WAIT}");
        value = engine.call("GET", &path, None, Duration::from_secs(WAIT + 10))?;
    }
    reply(&value)
}

fn steer(state: &Path, session: &str) -> Result<Value, String> {
    let engine = Engine::find(state)?;
    let value = engine.call(
        "POST",
        &format!("/api/sessions/{session}/steer"),
        Some(&json!({})),
        Duration::from_secs(10),
    )?;
    let steps: Vec<Value> = value["messages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(|m| json!({"userMessage": m}))
        .collect();
    Ok(if steps.is_empty() {
        json!({})
    } else {
        json!({"injectSteps": steps})
    })
}

/// No engine to ask (a CLI run, or the app is closed): decide with antiquota's own rules and
/// refuse what would need a yes, since nobody is there to give it.
fn decide_here(state: &Path, mode: Mode, tool: &str, args: &Value) -> Value {
    let permissions = state
        .parent()
        .map(|root| root.join("stack/.gemini/antigravity-cli/settings.json"))
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .map(|settings| {
            let list = |key: &str| -> Vec<String> {
                settings["permissions"][key]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            };
            Permissions {
                allow: list("allow"),
                deny: list("deny"),
            }
        })
        .unwrap_or_default();
    let decision = policy::decide(mode, tool, args, &permissions);
    match decision.verdict {
        Verdict::Allow => json!({"decision": "allow"}),
        Verdict::Deny => deny(&decision.reason),
        Verdict::Ask => deny(NO_APPROVER),
    }
}

/// The engine this turn belongs to, found through its discovery file and GUI token.
struct Engine {
    port: u16,
    token: String,
}
impl Engine {
    fn find(state: &Path) -> Result<Self, String> {
        let info: Value = std::fs::read(state.join("engine.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .ok_or("no running engine")?;
        let port = info["port"]
            .as_u64()
            .and_then(|p| u16::try_from(p).ok())
            .ok_or("engine discovery has no port")?;
        let token = std::fs::read_to_string(state.join("gui-token"))
            .map_err(|_| "engine token is unreadable")?
            .trim()
            .to_owned();
        Ok(Self { port, token })
    }

    fn call(
        &self,
        method: &str,
        path: &str,
        body: Option<&Value>,
        timeout: Duration,
    ) -> Result<Value, String> {
        let address = SocketAddr::from(([127, 0, 0, 1], self.port));
        let mut socket = TcpStream::connect_timeout(&address, Duration::from_secs(2))
            .map_err(|e| format!("engine unreachable: {e}"))?;
        socket
            .set_read_timeout(Some(timeout))
            .and_then(|_| socket.set_write_timeout(Some(Duration::from_secs(5))))
            .map_err(|e| e.to_string())?;
        let body = body.map(Value::to_string).unwrap_or_default();
        let head = format!(
            "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nX-Antiquota-Token: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            self.port,
            self.token,
            body.len()
        );
        socket
            .write_all(head.as_bytes())
            .and_then(|_| socket.write_all(body.as_bytes()))
            .map_err(|e| format!("engine request failed: {e}"))?;
        let mut bytes = vec![];
        socket
            .take(8_000_000)
            .read_to_end(&mut bytes)
            .map_err(|e| format!("engine reply failed: {e}"))?;
        let split = bytes
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .ok_or("engine reply is malformed")?;
        let status = std::str::from_utf8(&bytes[..split])
            .ok()
            .and_then(|head| head.split_whitespace().nth(1))
            .and_then(|code| code.parse::<u16>().ok())
            .ok_or("engine reply has no status")?;
        let value: Value =
            serde_json::from_slice(&bytes[split + 4..]).map_err(|_| "engine reply is not JSON")?;
        if status != 200 {
            return Err(format!(
                "engine answered {status}: {}",
                value["error"].as_str().unwrap_or("")
            ));
        }
        Ok(value)
    }
}

#[cfg(test)]
mod tests;
