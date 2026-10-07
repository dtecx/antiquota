//! Owns the legacy HTTP routes, security checks, and response envelopes.
use crate::{api::*, runtime::HttpState};
use antiquota_app::AppError;
use antiquota_core::{Command, Mode, SessionPatch};
use axum::{
    body::{Body, to_bytes},
    extract::{Request, State},
    http::{StatusCode, header},
    response::Response,
};
use serde::Serialize;
use serde_json::{Value, json};
use std::{path::Path, sync::Arc};
pub async fn dispatch(State(state): State<Arc<HttpState>>, request: Request) -> Response {
    let method = request.method().as_str().to_owned();
    let path = request.uri().path().to_owned();
    let query = request.uri().query().unwrap_or("").to_owned();
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|x| x.to_str().ok())
        .unwrap_or("");
    if host != format!("127.0.0.1:{}", state.port) && host != format!("localhost:{}", state.port) {
        return error(403, "bad host");
    }
    if method == "GET" && path == "/api/health" {
        return response(
            200,
            &HealthResponse {
                ok: true,
                app: "antiquota".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                pid: std::process::id(),
            },
        );
    }
    if method == "GET"
        && let Some(file) = static_path(&path)
    {
        return static_response(&state, &file);
    }
    let terminal_stream = method == "GET"
        && path.starts_with("/api/sessions/")
        && path.ends_with("/term/stream")
        && path.matches('/').count() == 5;
    let attachment_download = method == "GET"
        && path.starts_with("/api/sessions/")
        && path.split('/').nth(4) == Some("attachments")
        && path.split('/').count() == 6;
    let token = if path == "/api/events" || terminal_stream || attachment_download {
        query_token(request.uri().query().unwrap_or(""))
    } else {
        request
            .headers()
            .get("X-Antiquota-Token")
            .and_then(|x| x.to_str().ok())
            .unwrap_or("")
            .to_owned()
    };
    if !path.starts_with("/api/") || !equal_token(token.as_bytes(), state.token.as_bytes()) {
        return error(403, "forbidden");
    }
    if method == "GET" && path == "/api/events" {
        return crate::stream::events(&state);
    }
    if attachment_download {
        let parts: Vec<&str> = path.split('/').collect();
        let (id, name) = (parts[3], decode(parts[5]));
        return match state.service.read_attachment(id, &name) {
            Ok(Some(bytes)) => bytes_response(200, bytes, mime(&name)),
            Ok(None) => error(404, &format!("not found: '{name}'")),
            Err(AppError::Missing(x)) => error(404, &format!("not found: '{x}'")),
            Err(e) => error(400, &e.to_string()),
        };
    }
    let declared = request
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|x| x.to_str().ok())
        .and_then(|x| x.parse::<usize>().ok());
    let raw_target = match (method.as_str(), path.as_str()) {
        ("POST", "/api/transcribe") => Some(("recording too long", None)),
        ("POST", p)
            if p.starts_with("/api/sessions/")
                && p.ends_with("/attach")
                && p.matches('/').count() == 4 =>
        {
            Some((
                "attachments are limited to 25 MB",
                Some(p.split('/').nth(3).unwrap_or("").to_owned()),
            ))
        }
        _ => None,
    };
    if let Some((too_large, session)) = raw_target {
        let limit = antiquota_app::RECORDING_LIMIT;
        if declared.is_some_and(|n| n > limit) {
            return error(400, too_large);
        }
        // Read the headers before the body: a borrowed request cannot cross the await below.
        let (audio, filename) = {
            let headers = request.headers();
            let text = |name: &str| {
                headers
                    .get(name)
                    .and_then(|x| x.to_str().ok())
                    .map(str::to_owned)
            };
            (
                text("X-Audio-Type").unwrap_or_else(|| "wav".into()),
                text("X-Filename")
                    .map(|f| decode(&f))
                    .unwrap_or_else(|| "file".into()),
            )
        };
        let Ok(bytes) = to_bytes(request.into_body(), limit).await else {
            return error(400, too_large);
        };
        let result = match session {
            None => state
                .service
                .transcribe(&bytes, &audio)
                .await
                .map(|t| json!({"text": t.text, "slot": t.slot})),
            Some(id) => state.service.attach(&id, &filename, &bytes).and_then(value),
        };
        return match result {
            Ok(value) => response(200, &value),
            Err(AppError::Missing(x)) => error(404, &format!("not found: '{x}'")),
            Err(AppError::Invalid(x))
            | Err(AppError::Port(antiquota_core::PortError::Invalid(x))) => error(400, &x),
            Err(e) => error(500, &e.to_string()),
        };
    }
    if terminal_stream {
        let id = path
            .trim_start_matches("/api/sessions/")
            .trim_end_matches("/term/stream");
        return match state.service.term_feed(id) {
            Ok((info, feed)) => crate::stream::terminal(info, feed),
            Err(AppError::Missing(x)) => error(404, &format!("not found: '{x}'")),
            Err(e) => error(400, &e.to_string()),
        };
    }
    let body = if matches!(method.as_str(), "POST" | "PATCH" | "PUT") {
        if request
            .headers()
            .get(header::CONTENT_LENGTH)
            .and_then(|x| x.to_str().ok())
            .and_then(|x| x.parse::<usize>().ok())
            .is_some_and(|x| x > 5_000_000)
        {
            return error(400, "body too large");
        }
        match to_bytes(request.into_body(), 5_000_000).await {
            Ok(bytes) => {
                if bytes.is_empty() {
                    json!({})
                } else {
                    match serde_json::from_slice::<Value>(&bytes) {
                        Ok(value) => value,
                        Err(e) => return error(400, &json_error(&e)),
                    }
                }
            }
            Err(_) => return error(400, "body too large"),
        }
    } else {
        json!({})
    };
    let parts: Vec<_> = path.trim_start_matches("/api/").split('/').collect();
    let result = route(&state, &method, &parts, &body, &query).await;
    match result {
        Ok(value) => response(200, &value),
        Err(AppError::Missing(x)) => error(404, &format!("not found: '{x}'")),
        Err(AppError::Invalid(x)) | Err(AppError::Port(antiquota_core::PortError::Invalid(x))) => {
            error(400, &x)
        }
        Err(e) => {
            tracing::error!(error=%e,"request failed");
            error(500, &e.to_string())
        }
    }
}
async fn route(
    state: &Arc<HttpState>,
    method: &str,
    parts: &[&str],
    body: &Value,
    query: &str,
) -> Result<Value, AppError> {
    let service = &state.service;
    if let Some(result) = crate::account_http::route(service, method, parts, body, query).await {
        return result;
    }
    match (method, parts) {
        ("GET", ["state"]) => {
            let settings = service.public_defaults()?;
            let cwd = if settings.default_cwd.is_empty() {
                antiquota_store::user_home()?.to_string_lossy().into_owned()
            } else {
                crate::expand_cwd(&settings.default_cwd)?
            };
            value(StateResponse {
                sessions: service.sessions()?,
                accounts: service.account_overview()?,
                models: service.accounts.cached_models()?,
                modes: vec![Mode::Default, Mode::AcceptEdits, Mode::Plan, Mode::Bypass],
                defaults: RuntimeDefaults { settings, cwd },
                engine: EngineSummary {
                    version: env!("CARGO_PKG_VERSION").into(),
                    agy: state.agy.to_string_lossy().into_owned(),
                    busy: service.busy_count(),
                },
            })
        }
        ("POST", ["sessions"]) => {
            let worktree = body.get("worktree").is_some_and(truthy);
            let defaults = service.public_defaults()?;
            let cwd = body
                .get("cwd")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| {
                    if defaults.default_cwd.is_empty() {
                        antiquota_store::user_home()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned()
                    } else {
                        defaults.default_cwd.clone()
                    }
                });
            let mode = antiquota_app::parse_mode(
                body.get("mode")
                    .and_then(Value::as_str)
                    .unwrap_or(&defaults.mode),
            )?;
            let title = body["title"].as_str().unwrap_or("");
            let model = body
                .get("model")
                .and_then(Value::as_str)
                .unwrap_or(&defaults.model);
            if worktree {
                return value(service.create_in(&cwd, title, model, mode, true)?);
            }
            command(
                service,
                Command::Create {
                    cwd,
                    title: title.into(),
                    model: model.into(),
                    mode,
                },
            )
            .await
        }
        ("GET", ["sessions", id]) => {
            let (meta, events) = service.get(id)?;
            value(SessionResponse { meta, events })
        }
        ("PATCH", ["sessions", id]) => {
            command(
                service,
                Command::Patch {
                    session: (*id).into(),
                    fields: patch_fields(body)?,
                },
            )
            .await
        }
        ("DELETE", ["sessions", id]) => {
            command(
                service,
                Command::Delete {
                    session: (*id).into(),
                },
            )
            .await
        }
        ("POST", ["sessions", id, "send"]) => {
            let text = body["text"].as_str().unwrap_or("");
            let attachments: Vec<String> = body["attachments"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect();
            if text.trim().is_empty() && attachments.is_empty() {
                return Err(AppError::Invalid("empty message".into()));
            }
            command(
                service,
                Command::Send {
                    session: (*id).into(),
                    text: text.into(),
                    attachments,
                    kind: "user".into(),
                },
            )
            .await
        }
        ("POST", ["sessions", id, "stop"]) => {
            command(
                service,
                Command::Stop {
                    session: (*id).into(),
                },
            )
            .await
        }
        ("POST", ["approvals"]) => {
            let text = |key: &str| match body.get(key) {
                None | Some(Value::Null) => String::new(),
                Some(Value::String(s)) => s.clone(),
                Some(other) => other.to_string(),
            };
            let args = match body.get("args") {
                Some(args) if truthy(args) => args.clone(),
                _ => json!({}),
            };
            approval(service.approval_request(&text("session"), &text("tool"), &args)?)
        }
        ("GET", ["approvals", id]) => {
            let wait = query_value(query, "wait").unwrap_or_else(|| "25".into());
            let wait: f64 = wait.trim().parse().map_err(|_| {
                AppError::Invalid(format!("could not convert string to float: '{wait}'"))
            })?;
            approval(service.approval_wait(id, wait).await?)
        }
        ("POST", ["approvals", id, "answer"]) => {
            let decision = body.get("decision").and_then(Value::as_str);
            if !matches!(decision, Some("allow" | "deny")) {
                return Err(AppError::Invalid("decision: allow or deny".into()));
            }
            let why: String = match body.get("why") {
                None | Some(Value::Null) => String::new(),
                Some(Value::String(s)) => s.clone(),
                Some(other) => other.to_string(),
            }
            .chars()
            .take(500)
            .collect();
            let always = body.get("always").is_some_and(truthy);
            approval(service.approval_answer(id, decision == Some("allow"), always, &why)?)
        }
        ("POST", ["sessions", id, "goal"]) => {
            let text = |key: &str| match body.get(key) {
                None | Some(Value::Null) => String::new(),
                Some(Value::String(s)) => s.clone(),
                Some(other) => other.to_string(),
            };
            let max_rounds = match body.get("max_rounds") {
                None => 20,
                Some(Value::Number(n)) if n.as_f64().is_some_and(|f| f >= 0.0) => {
                    n.as_f64().unwrap_or(20.0) as u64
                }
                Some(Value::String(s)) if s.trim().parse::<u64>().is_ok() => {
                    s.trim().parse().unwrap_or(20)
                }
                Some(_) => {
                    return Err(AppError::Invalid(
                        "max_rounds: a whole number of rounds".into(),
                    ));
                }
            };
            let session = service.set_goal(
                id,
                &text("objective"),
                &text("check"),
                body.get("verifier").is_none_or(truthy),
                &text("verifier_model"),
                max_rounds,
                true,
            )?;
            value(session)
        }
        ("POST", ["sessions", id, "goal", "resume"]) => {
            service.resume_goal(id)?;
            value(OkResponse { ok: true })
        }
        ("DELETE", ["sessions", id, "goal"]) => {
            service.clear_goal(id)?;
            value(OkResponse { ok: true })
        }
        ("POST", ["sessions", id, "jobs"]) => {
            let command = loose_text(body.get("cmd"));
            if command.trim().is_empty() {
                return Err(AppError::Invalid("empty command".into()));
            }
            let every = antiquota_core::jobs::parse_interval(&loose_text(body.get("every")), 'm');
            let notify = body.get("notify").is_none_or(truthy);
            value(service.job_start(id, command.trim(), every, notify, "user")?)
        }
        ("POST", ["sessions", id, "jobs", jid, "stop"]) => value(service.job_stop(id, jid)?),
        ("GET", ["sessions", id, "jobs", jid, "log"]) => {
            let (job, text) = service.job_log(id, jid)?;
            value(JobLogResponse { job, text })
        }
        ("POST", ["sessions", id, "loop"]) => {
            let at = body
                .get("at")
                .filter(|v| truthy(v))
                .map(|v| loose_text(Some(v)));
            let prompt = loose_text(body.get("prompt"));
            if let Some(at) = at {
                let next = service.set_loop(id, 0, &prompt, Some(&at))?;
                return Ok(json!({"ok": true, "next": next}));
            }
            let every = antiquota_core::jobs::parse_interval(&loose_text(body.get("every")), 'm');
            if every == 0 {
                return Err(AppError::Invalid("interval like 15m, 1h or 90s".into()));
            }
            service.set_loop(id, every, &prompt, None)?;
            value(OkResponse { ok: true })
        }
        ("DELETE", ["sessions", id, "loop"]) => {
            service.set_loop(id, 0, "", None)?;
            value(OkResponse { ok: true })
        }
        ("GET", ["sessions", id, "tree"]) => {
            service.project_tree(id, &query_value(query, "path").unwrap_or_default())
        }
        ("GET", ["sessions", id, "file"]) => {
            service.read_file(id, &query_value(query, "path").unwrap_or_default())
        }
        ("GET", ["sessions", id, "git"]) => service.git_status(id),
        ("GET", ["sessions", id, "git", "diff"]) => {
            let path = query_value(query, "path").filter(|p| !p.is_empty());
            let committed = query_value(query, "committed").as_deref() == Some("1");
            Ok(json!({"diff": service.git_diff(id, path.as_deref(), committed)?}))
        }
        ("POST", ["sessions", id, "git", "commit"]) => {
            Ok(json!({"sha": service.git_commit(id, &loose_text(body.get("message")))?}))
        }
        ("POST", ["sessions", id, "git", "discard"]) => {
            service.git_discard(id, &loose_text(body.get("path")))?;
            value(OkResponse { ok: true })
        }
        ("POST", ["sessions", id, "git", "merge"]) => Ok(json!({"sha": service.git_merge(id)?})),
        ("POST", ["sessions", id, "git", "remove"]) => {
            value(service.git_remove_worktree(id, body.get("delete_branch").is_some_and(truthy))?)
        }
        ("POST", ["sessions", id, "term"]) => {
            let (cols, rows) = (dimension(body.get("cols"))?, dimension(body.get("rows"))?);
            let restart = body.get("restart").is_some_and(truthy);
            value(service.term_start(id, cols.unwrap_or(100), rows.unwrap_or(30), restart)?)
        }
        ("POST", ["sessions", id, "term", "input"]) => {
            service.term_input(id, &loose_text(body.get("data")))?;
            value(OkResponse { ok: true })
        }
        ("POST", ["sessions", id, "term", "resize"]) => {
            let (cols, rows) = (dimension(body.get("cols"))?, dimension(body.get("rows"))?);
            service.term_resize(id, cols.unwrap_or(100), rows.unwrap_or(30))?;
            value(OkResponse { ok: true })
        }
        ("POST", ["sessions", id, "term", "kill"]) => {
            service.term_kill(id);
            value(OkResponse { ok: true })
        }
        ("POST", ["sessions", id, "steer"]) => value(SteerResponse {
            messages: service.steer(id)?,
        }),
        ("GET", ["permissions"]) => value(AllowRulesResponse {
            allow: service.accounts.permissions()?.allow,
        }),
        ("PUT", ["permissions"]) => {
            let list = |key: &str| -> Result<Option<Vec<String>>, AppError> {
                match body.get(key) {
                    None | Some(Value::Null) => Ok(None),
                    Some(rules) => antiquota_core::policy::clean_rules(key, rules)
                        .map(Some)
                        .map_err(AppError::Invalid),
                }
            };
            let (allow, deny) = (list("allow")?, list("deny")?);
            Ok(service.accounts.save_permissions(allow, deny)?)
        }
        ("POST", ["shutdown"]) => {
            state.closing.send_replace(true);
            state.shutdown.notify_one();
            value(OkResponse { ok: true })
        }
        _ => Err(AppError::Missing(parts.join("/"))),
    }
}
pub(crate) async fn command(
    service: &Arc<antiquota_app::Service>,
    command: Command,
) -> Result<Value, AppError> {
    match service.execute(command).await? {
        antiquota_app::CommandResult::Session(session) => value(*session),
        antiquota_app::CommandResult::Sent(result) => value(SendResponse { result }),
        antiquota_app::CommandResult::Value(data) => Ok(data),
        antiquota_app::CommandResult::Done => value(OkResponse { ok: true }),
    }
}
fn patch_fields(body: &Value) -> Result<SessionPatch, AppError> {
    fn text(body: &Value, key: &str) -> Result<Option<String>, AppError> {
        body.get(key)
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| AppError::Invalid(format!("{key} must be a string")))
            })
            .transpose()
    }
    Ok(SessionPatch {
        title: text(body, "title")?,
        model: text(body, "model")?,
        effort: text(body, "effort")?,
        mode: body
            .get("mode")
            .map(|value| antiquota_app::parse_mode(value.as_str().unwrap_or("")))
            .transpose()?,
        pinned: body.get("pinned").map(truthy),
        archived: body.get("archived").map(truthy),
    })
}
/// A terminal size from a JSON number or numeric string, clamped to the PTY range.
fn dimension(value: Option<&Value>) -> Result<Option<u16>, AppError> {
    let number = match value {
        None | Some(Value::Null) => return Ok(None),
        Some(Value::Number(n)) => n.as_f64(),
        Some(Value::String(s)) => s.trim().parse::<f64>().ok(),
        Some(_) => None,
    };
    number
        .filter(|n| n.is_finite())
        .map(|n| Some(n.clamp(0.0, f64::from(u16::MAX)) as u16))
        .ok_or_else(|| AppError::Invalid("terminal size must be a number".into()))
}
/// Python's `str(value or "")` for loosely typed request fields.
fn loose_text(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Bool(false)) => String::new(),
        Some(Value::Number(n)) if n.as_f64() == Some(0.0) => String::new(),
        Some(other) => other.to_string(),
    }
}
fn approval(reply: antiquota_app::ApprovalReply) -> Result<Value, AppError> {
    value(match reply {
        antiquota_app::ApprovalReply::Pending(id) => ApprovalResponse {
            pending: Some(id),
            ..Default::default()
        },
        antiquota_app::ApprovalReply::Decided { decision, reason } => ApprovalResponse {
            decision: Some(decision.as_str().into()),
            reason: (!reason.is_empty()).then_some(reason),
            ..Default::default()
        },
    })
}
/// One decoded query parameter, like Python's `parse_qs`.
fn query_value(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|part| {
        let (k, v) = part.split_once('=').unwrap_or((part, ""));
        (decode(k) == key).then(|| decode(v))
    })
}
fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len()
                && bytes[i + 1].is_ascii_hexdigit()
                && bytes[i + 2].is_ascii_hexdigit() =>
            {
                let hex = |b: u8| (b as char).to_digit(16).unwrap_or(0) as u8;
                out.push(hex(bytes[i + 1]) * 16 + hex(bytes[i + 2]));
                i += 2;
            }
            other => out.push(other),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
pub(crate) fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(x) => *x,
        Value::Number(n) => n.as_f64() != Some(0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(x) => !x.is_empty(),
        Value::Object(x) => !x.is_empty(),
    }
}
pub(crate) fn value<T: Serialize>(item: T) -> Result<Value, AppError> {
    serde_json::to_value(item).map_err(|e| AppError::Invalid(e.to_string()))
}
pub fn response<T: Serialize>(code: u16, value: &T) -> Response {
    let body = serde_json::to_vec(value)
        .unwrap_or_else(|_| b"{\"error\":\"serialization failed\"}".to_vec());
    bytes_response(code, body, "application/json")
}
pub fn error(code: u16, text: &str) -> Response {
    response(code, &ApiError { error: text.into() })
}
fn bytes_response(code: u16, bytes: Vec<u8>, kind: &str) -> Response {
    Response::builder()
        .status(StatusCode::from_u16(code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
        .header(header::CONTENT_TYPE, kind)
        .header(header::CONTENT_LENGTH, bytes.len())
        .header(header::CACHE_CONTROL, "no-store")
        .header("X-Content-Type-Options", "nosniff")
        .header(header::SERVER, "antiquota/0.5.0")
        .body(Body::from(bytes))
        .unwrap_or_else(|_| Response::new(Body::empty()))
}
/// The content type of an attachment by its extension, as Python's mimetypes guesses it.
fn mime(name: &str) -> &'static str {
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "txt" | "log" => "text/plain",
        "md" => "text/markdown",
        "csv" => "text/csv",
        "html" | "htm" => "text/html",
        "json" => "application/json",
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "heic" => "image/heic",
        "svg" => "image/svg+xml",
        "wav" => "audio/x-wav",
        "zip" => "application/zip",
        _ => "application/octet-stream",
    }
}
fn static_path(path: &str) -> Option<String> {
    match path {
        "/" | "/index.html" => Some("index.html".into()),
        "/app.js" | "/app.css" => Some(path[1..].into()),
        p if p.starts_with("/assets/") && !p.contains("..") => Some(p[1..].into()),
        _ => None,
    }
}
fn static_response(state: &HttpState, file: &str) -> Response {
    let root = state.ui_dir.canonicalize();
    let path = state.ui_dir.join(file).canonicalize();
    let (Ok(root), Ok(path)) = (root, path) else {
        return error(404, &format!("not found: '{file}'"));
    };
    if !path.starts_with(root) || !path.is_file() {
        return error(403, "forbidden");
    }
    let Ok(mut bytes) = std::fs::read(&path) else {
        return error(404, &format!("not found: '{file}'"));
    };
    if file == "index.html" {
        bytes = String::from_utf8_lossy(&bytes)
            .replace("__ANTIQUOTA_TOKEN__", &state.token)
            .into_bytes();
    }
    let kind = match Path::new(file).extension().and_then(|x| x.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("svg") => "image/svg+xml",
        _ => "application/octet-stream",
    };
    bytes_response(200, bytes, kind)
}
fn query_token(query: &str) -> String {
    query
        .split('&')
        .find_map(|part| part.strip_prefix("token="))
        .unwrap_or("")
        .to_owned()
}
fn equal_token(left: &[u8], right: &[u8]) -> bool {
    let mut diff = left.len() ^ right.len();
    for (a, b) in left.iter().zip(right) {
        diff |= usize::from(a ^ b);
    }
    diff == 0
}
fn json_error(error: &serde_json::Error) -> String {
    let line = error.line();
    let column = if error.is_eof() {
        error.column() + 1
    } else {
        error.column()
    };
    let text = error.to_string();
    let reason = if text.starts_with("key must be a string")
        || text.starts_with("EOF while parsing an object")
    {
        "Expecting property name enclosed in double quotes"
    } else if text.starts_with("EOF") || text.starts_with("expected value") {
        "Expecting value"
    } else {
        "Invalid JSON"
    };
    format!(
        "{reason}: line {line} column {column} (char {})",
        column.saturating_sub(1)
    )
}
