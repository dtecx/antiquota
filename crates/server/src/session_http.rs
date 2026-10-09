//! Owns the session, approval, goal, job, loop, file, git, terminal and permission routes.
use crate::{
    api::*,
    http::{command, loose_text, query_value, truthy, value},
    runtime::HttpState,
};
use antiquota_app::AppError;
use antiquota_core::{Command, Mode, SessionPatch};
use serde_json::{Value, json};
use std::sync::Arc;
pub(crate) async fn route(
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
    if let Some(result) = crate::parity_http::route(state, method, parts, body, query).await {
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
            if let Some(rules) = body.get("add_allow") {
                if body.get("allow").is_some() || body.get("deny").is_some() {
                    return Err(AppError::Invalid(
                        "add_allow cannot be combined with replacements".into(),
                    ));
                }
                let rules = antiquota_core::policy::clean_rules("allow", rules)
                    .map_err(AppError::Invalid)?;
                return Ok(service.accounts.grant_permissions(rules)?);
            }
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
