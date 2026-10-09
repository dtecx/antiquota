//! Owns the HTTP boundary: host and token checks, raw uploads, static files, request
//! decoding and response envelopes. Route groups live in their own modules.
use crate::{api::*, runtime::HttpState};
use antiquota_app::AppError;
use antiquota_core::Command;
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
    let result = crate::session_http::route(&state, &method, &parts, &body, &query).await;
    match result {
        Ok(value) => response(200, &value),
        Err(AppError::Missing(x)) => error(404, &format!("not found: '{x}'")),
        Err(AppError::Invalid(x)) | Err(AppError::Port(antiquota_core::PortError::Invalid(x))) => {
            error(400, &x)
        }
        Err(AppError::Port(missing @ antiquota_core::PortError::NoFile(_))) => {
            error(404, &missing.to_string())
        }
        Err(e) => {
            tracing::error!(error=%e,"request failed");
            error(500, &e.to_string())
        }
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
/// Python's `str(value or "")` for loosely typed request fields.
pub(crate) fn loose_text(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Bool(false)) => String::new(),
        Some(Value::Number(n)) if n.as_f64() == Some(0.0) => String::new(),
        Some(other) => other.to_string(),
    }
}
/// One decoded query parameter, like Python's `parse_qs`.
pub(crate) fn query_value(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|part| {
        let (k, v) = part.split_once('=').unwrap_or((part, ""));
        (decode(k) == key).then(|| decode(v))
    })
}
pub(crate) fn decode(text: &str) -> String {
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
