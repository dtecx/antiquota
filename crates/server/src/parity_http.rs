//! Translates the remaining captured session, settings and diagnostics HTTP requests.
use crate::{
    http::{loose_text, query_value, truthy},
    runtime::HttpState,
};
use antiquota_app::AppError;
use serde_json::{Value, json};
use std::sync::Arc;

pub async fn route(
    state: &Arc<HttpState>,
    method: &str,
    parts: &[&str],
    body: &Value,
    query: &str,
) -> Option<Result<Value, AppError>> {
    let service = &state.service;
    Some(match (method, parts) {
        ("GET", ["search"]) => service.search(&query_value(query, "q").unwrap_or_default()),
        ("POST", ["sessions", id, "fork"]) => service.fork(id).and_then(crate::http::value),
        ("GET", ["sessions", id, "state"]) => {
            service.state_document(id).map(|text| json!({"text":text}))
        }
        ("POST", ["sessions", id, "inherit"]) => service
            .inherit(id, &loose_text(body.get("from")))
            .map(|()| json!({"ok":true})),
        ("POST", ["sessions", id, "clear"]) => {
            service.clear(id, false).map(|()| json!({"ok":true}))
        }
        ("POST", ["sessions", id, "compact"]) => {
            service.clear(id, true).map(|()| json!({"ok":true}))
        }
        ("POST", ["sessions", id, "rewind"]) => {
            let turn = body
                .get("turn")
                .map_or(Some(0), |v| v.as_u64().or_else(|| v.as_str()?.parse().ok()));
            match turn {
                Some(turn) => service.rewind(id, turn).map(|files| json!({"files":files})),
                None => Err(AppError::Invalid(
                    "turn must be a nonnegative integer".into(),
                )),
            }
        }
        ("POST", ["sessions", id, "shell"]) => {
            service.shell(id, &loose_text(body.get("cmd"))).await
        }
        ("POST", ["sessions", id, "note"]) => service.note(id, &loose_text(body.get("text"))),
        ("GET", ["sessions", id, "files"]) => service
            .mention_files(id, &query_value(query, "q").unwrap_or_default())
            .map(|files| json!(files)),
        ("GET", ["fs"]) => service.browse(&query_value(query, "path").unwrap_or_default()),
        ("GET", ["usage"]) => match query_value(query, "days")
            .unwrap_or_else(|| "14".into())
            .parse::<i64>()
        {
            Ok(days) => service
                .store
                .usage_history(
                    days.clamp(1, 90) as u32,
                    antiquota_core::Clock::now(&antiquota_app::SystemClock),
                )
                .map_err(AppError::from),
            Err(_) => Err(AppError::Invalid("days must be an integer".into())),
        },
        ("GET", ["engine"]) => engine(state),
        ("GET", ["engine", "log"]) => service
            .store
            .engine_log()
            .map(|text| json!({"text":text}))
            .map_err(AppError::from),
        ("GET", ["settings", "file"]) => service
            .accounts
            .stack_file(&query_value(query, "path").unwrap_or_default())
            .map_err(AppError::from),
        ("PUT", ["settings", "file"]) => match body["text"].as_str() {
            Some(text) => service
                .accounts
                .write_stack_file(&loose_text(body.get("path")), text)
                .map(|()| json!({"ok":true}))
                .map_err(AppError::from),
            None => Err(AppError::Invalid("text too large".into())),
        },
        ("POST", ["settings", "copy"]) => service
            .accounts
            .copy_stack_item(
                &loose_text(body.get("source")),
                &loose_text(body.get("kind")),
                body["name"].as_str().filter(|s| !s.is_empty()),
            )
            .map(|()| json!({"ok":true}))
            .map_err(AppError::from),
        ("GET", ["settings", kind, name]) => service
            .accounts
            .stack_item(kind, &crate::http::decode(name))
            .map_err(item_error),
        ("PUT", ["settings", kind, name]) => service
            .accounts
            .put_stack_item(kind, &crate::http::decode(name), &body["value"])
            .map(|()| json!({"ok":true}))
            .map_err(item_error),
        ("DELETE", ["settings", kind, name]) => service
            .accounts
            .delete_stack_item(kind, &crate::http::decode(name))
            .map(|()| json!({"ok":true}))
            .map_err(item_error),
        ("POST", ["settings", kind, name, "enabled"]) => service
            .accounts
            .enable_stack_item(
                kind,
                &crate::http::decode(name),
                body.get("on").is_some_and(truthy),
            )
            .await
            .map(|()| json!({"ok":true}))
            .map_err(item_error),
        ("GET", ["stack"]) => service
            .accounts
            .stack_info(false)
            .await
            .map_err(AppError::from),
        ("POST", ["stack", "reseed"]) => service
            .accounts
            .stack_info(true)
            .await
            .map_err(AppError::from),
        _ => return None,
    })
}
fn item_error(e: antiquota_core::PortError) -> AppError {
    if let antiquota_core::PortError::Invalid(text) = &e
        && let Some(name) = text
            .strip_prefix("not found: '")
            .and_then(|s| s.strip_suffix('\''))
    {
        return AppError::Missing(name.into());
    }
    e.into()
}
fn engine(state: &HttpState) -> Result<Value, AppError> {
    let mut info = state.service.store.engine_details()?;
    info["version"] = json!(env!("CARGO_PKG_VERSION"));
    info["pid"] = json!(std::process::id());
    info["agy"] = json!(state.agy.to_string_lossy());
    info["sessions"] = json!(state.service.sessions()?.len());
    info["accounts"] = json!(state.service.account_overview()?.len());
    Ok(info)
}
