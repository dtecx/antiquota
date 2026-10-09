//! Owns the compatible account, login, routing and defaults HTTP routes.
use antiquota_app::{AppError, Service};
use antiquota_core::{AccountCommand, Command};
use serde_json::{Value, json};
use std::sync::Arc;
pub async fn route(
    service: &Arc<Service>,
    method: &str,
    parts: &[&str],
    body: &Value,
    query: &str,
) -> Option<Result<Value, AppError>> {
    let action = match (method, parts) {
        ("GET", ["accounts"]) => {
            return Some(service.account_overview().and_then(crate::http::value));
        }
        ("GET", ["routing"]) => {
            return Some((|| {
                let text = query
                    .split('&')
                    .find_map(|p| p.strip_prefix("n="))
                    .filter(|n| !n.is_empty())
                    .unwrap_or("60");
                let n = text
                    .parse::<i64>()
                    .map_err(|_| AppError::Invalid("n must be an integer".into()))?;
                let rows = service
                    .accounts
                    .routes(if n <= 0 { usize::MAX } else { n as usize })?;
                let rows = if n < 0 {
                    rows.into_iter().skip(n.unsigned_abs() as usize).collect()
                } else {
                    rows
                };
                crate::http::value(rows)
            })());
        }
        ("POST", ["accounts", "probe"]) => AccountCommand::Probe,
        ("POST", ["accounts", "login"]) => AccountCommand::LoginStart,
        ("GET", ["accounts", "login", id]) => {
            return Some(
                service
                    .accounts
                    .login_state(id)
                    .map_err(AppError::from)
                    .and_then(crate::http::value),
            );
        }
        ("POST", ["accounts", "login", id]) => AccountCommand::LoginSubmit {
            id: (*id).into(),
            code: body["code"].as_str().unwrap_or("").into(),
        },
        ("DELETE", ["accounts", "login", id]) => AccountCommand::LoginCancel { id: (*id).into() },
        ("POST", ["accounts", name, "check"]) => match name_value(name) {
            Ok(name) => AccountCommand::Check { name },
            Err(error) => return Some(Err(error)),
        },
        ("POST", ["accounts", name, "pause"]) => match name_value(name) {
            Ok(name) => AccountCommand::Pause {
                name,
                on: body.get("on").map(crate::http::truthy).unwrap_or(true),
            },
            Err(error) => return Some(Err(error)),
        },
        ("DELETE", ["accounts", name]) => match name_value(name) {
            Ok(name) => AccountCommand::Remove { name },
            Err(error) => return Some(Err(error)),
        },
        ("GET", ["settings"]) => {
            return Some(
                async {
                    let mut result = service.accounts.stack_overview().await?;
                    result["defaults"] = json!(service.public_defaults()?);
                    Ok(result)
                }
                .await,
            );
        }
        ("PUT", ["settings", "defaults"]) => AccountCommand::Defaults {
            values: body.clone(),
        },
        _ => return None,
    };
    Some(crate::http::command(service, Command::Account { action }).await)
}
fn name_value(raw: &str) -> Result<String, AppError> {
    let bytes = raw.as_bytes();
    let mut result = vec![];
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%' && at + 2 < bytes.len() {
            let hex = |b: u8| char::from(b).to_digit(16).map(|n| n as u8);
            if let Some((a, b)) = hex(bytes[at + 1]).zip(hex(bytes[at + 2])) {
                result.push(a * 16 + b);
                at += 3;
                continue;
            }
        }
        result.push(bytes[at]);
        at += 1;
    }
    let name =
        String::from_utf8(result).map_err(|_| AppError::Invalid("invalid account name".into()))?;
    antiquota_core::validate_account_name(&name)?;
    Ok(name)
}
