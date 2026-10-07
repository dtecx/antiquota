//! Owns tool approvals: immediate policy decisions, pending cards, answers, timeouts and the
//! permanent rule or edit mode an "always" answer adds.
use crate::{AppError, Service, SessionState};
use antiquota_core::{
    Approval, Event, Mode, Verdict,
    policy::{self, DECLINED, STOPPED, WRITE_TOOLS},
};
use serde_json::{Value, json};
use std::time::Duration;

/// Longest single string kept in an approval card; longer text is cut with a visible note.
const KEEP_CHARS: usize = 4000;
const KEEP_ITEMS: usize = 200;
/// The longest one long-poll request waits before the hook asks again.
const MAX_WAIT: f64 = 60.0;

/// What the hook hears: a decision with an optional reason, or the id it should wait on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalReply {
    Decided { decision: Verdict, reason: String },
    Pending(String),
}
impl ApprovalReply {
    fn ask() -> Self {
        Self::Decided {
            decision: Verdict::Ask,
            reason: String::new(),
        }
    }
    fn of(approval: &Approval) -> Self {
        if approval.state == "allowed" {
            Self::Decided {
                decision: Verdict::Allow,
                reason: String::new(),
            }
        } else {
            Self::Decided {
                decision: Verdict::Deny,
                reason: if approval.why.is_empty() {
                    DECLINED.into()
                } else {
                    approval.why.clone()
                },
            }
        }
    }
}

/// Keeps nested arguments so the user can judge the call, cutting very long text visibly.
fn keep(value: &Value) -> Value {
    match value {
        Value::String(text) if text.chars().count() > KEEP_CHARS => {
            let cut: String = text.chars().take(KEEP_CHARS).collect();
            let more = text.chars().count() - KEEP_CHARS;
            Value::String(format!("{cut}\n… ({more} more characters)"))
        }
        Value::Array(items) => {
            let mut kept: Vec<Value> = items.iter().take(KEEP_ITEMS).map(keep).collect();
            if items.len() > KEEP_ITEMS {
                kept.push(json!(format!(
                    "… ({} more items)",
                    items.len() - KEEP_ITEMS
                )));
            }
            Value::Array(kept)
        }
        Value::Object(map) => {
            Value::Object(map.iter().map(|(k, v)| (k.clone(), keep(v))).collect())
        }
        other => other.clone(),
    }
}

impl Service {
    fn approval_minutes(&self) -> f64 {
        self.public_defaults()
            .ok()
            .and_then(|s| s.approval_timeout.parse::<f64>().ok())
            .filter(|m| m.is_finite())
            .unwrap_or(15.0)
            .clamp(1.0, 240.0)
    }

    /// Decides a gated tool call at once from the session's mode and antiquota's rules, or
    /// parks it for the user. Unknown sessions get "ask": agy's own behaviour.
    pub fn approval_request(
        &self,
        session: &str,
        tool: &str,
        args: &Value,
    ) -> Result<ApprovalReply, AppError> {
        let Ok(cell) = self.cell(session) else {
            return Ok(ApprovalReply::ask());
        };
        let permissions = self.accounts.permissions()?;
        let minutes = self.approval_minutes();
        let mut state = cell
            .state
            .lock()
            .map_err(|_| AppError::Invalid("session unavailable".into()))?;
        if state.deleting {
            return Ok(ApprovalReply::ask());
        }
        let decision = policy::decide(state.meta.mode, tool, args, &permissions);
        if decision.verdict != Verdict::Ask {
            return Ok(ApprovalReply::Decided {
                decision: decision.verdict,
                reason: decision.reason,
            });
        }
        if state.cancel.is_cancelled() {
            return Ok(ApprovalReply::Decided {
                decision: Verdict::Deny,
                reason: STOPPED.into(),
            });
        }
        let id = uuid::Uuid::new_v4().simple().to_string()[..10].to_owned();
        let approval = Approval {
            id: id.clone(),
            session: session.into(),
            tool: tool.into(),
            args: keep(args),
            rule: decision.rule,
            state: "pending".into(),
            why: String::new(),
            deadline: self.clock.now() + minutes * 60.0,
            extra: Default::default(),
        };
        self.approval_map()?.insert(id.clone(), approval.clone());
        self.append(&mut state, Event::Approval { approval })?;
        let mut next = state.meta.clone();
        next.status = "approval".into();
        self.replace_meta(&mut state, next)?;
        Ok(ApprovalReply::Pending(id))
    }

    /// The decision, or the pending id again after `wait` seconds (the hook then asks again).
    /// An unanswered request is declined once its deadline passes.
    pub async fn approval_wait(&self, id: &str, wait: f64) -> Result<ApprovalReply, AppError> {
        let wait = if wait.is_finite() {
            wait.clamp(0.0, MAX_WAIT)
        } else {
            0.0
        };
        let until = tokio::time::Instant::now() + Duration::from_secs_f64(wait);
        loop {
            let changed = self.approval_signal.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let deadline = {
                let map = self.approval_map()?;
                match map.get(id) {
                    None => return Ok(ApprovalReply::ask()),
                    Some(approval) if approval.state != "pending" => {
                        return Ok(ApprovalReply::of(approval));
                    }
                    Some(approval) => approval.deadline,
                }
            };
            let now = self.clock.now();
            if now >= deadline {
                let minutes = self.approval_minutes();
                let why = format!(
                    "No answer from the user within {} minutes. Continue without it, or explain what you need.",
                    minutes as u64
                );
                return self.approval_answer(id, false, false, &why);
            }
            let left = until
                .saturating_duration_since(tokio::time::Instant::now())
                .min(Duration::from_secs_f64((deadline - now).max(0.0)))
                .min(Duration::from_secs(1));
            if left.is_zero() {
                return Ok(ApprovalReply::Pending(id.into()));
            }
            tokio::select! {
                _ = changed => {},
                _ = tokio::time::sleep(left) => {},
            }
        }
    }

    /// Answers a pending request once; later answers return the first decision. "Always" adds
    /// the suggested rule for every account and session, or lets this session make edits.
    pub fn approval_answer(
        &self,
        id: &str,
        allow: bool,
        always: bool,
        why: &str,
    ) -> Result<ApprovalReply, AppError> {
        let approval = {
            let mut map = self.approval_map()?;
            let approval = map
                .get_mut(id)
                .ok_or_else(|| AppError::Missing(id.into()))?;
            if approval.state != "pending" {
                return Ok(ApprovalReply::of(approval));
            }
            approval.state = if allow { "allowed" } else { "denied" }.into();
            approval.why = why.into();
            approval
                .extra
                .insert("answered".into(), json!(self.clock.now()));
            approval.extra.insert("always".into(), json!(always));
            approval.clone()
        };
        self.approval_signal.notify_waiters();
        let mut edits = false;
        let mut refused = None;
        if always && allow {
            if !approval.rule.is_empty() {
                match policy::permanent_rule_error(&approval.rule) {
                    None => self.accounts.add_allow_rule(&approval.rule)?,
                    Some(error) => refused = Some(error),
                }
            } else if WRITE_TOOLS.contains(&approval.tool.as_str()) {
                edits = true;
            }
        }
        if let Ok(cell) = self.cell(&approval.session) {
            let mut state = cell
                .state
                .lock()
                .map_err(|_| AppError::Invalid("session unavailable".into()))?;
            if !state.deleting {
                self.append(
                    &mut state,
                    Event::Approval {
                        approval: approval.clone(),
                    },
                )?;
                if let Some(text) = refused {
                    self.append(
                        &mut state,
                        Event::Notice {
                            kind: "approval".into(),
                            text: format!("Allowed once. {text}."),
                            data: json!({}),
                        },
                    )?;
                }
                let waiting = self
                    .approval_map()?
                    .values()
                    .any(|a| a.session == approval.session && a.state == "pending");
                let resume = state.meta.status == "approval" && !waiting;
                if edits || resume {
                    let mut next = state.meta.clone();
                    if edits {
                        next.mode = Mode::AcceptEdits;
                    }
                    if resume {
                        next.status = if state.busy { "running" } else { "idle" }.into();
                    }
                    self.replace_meta(&mut state, next)?;
                }
            }
        }
        Ok(ApprovalReply::of(&approval))
    }

    /// Declines every pending request of a stopped session and records each answer.
    pub(crate) fn deny_pending(&self, state: &mut SessionState) -> Result<(), AppError> {
        let declined: Vec<Approval> = {
            let mut map = self.approval_map()?;
            map.values_mut()
                .filter(|a| a.session == state.meta.id && a.state == "pending")
                .map(|a| {
                    a.state = "denied".into();
                    a.why = STOPPED.into();
                    a.clone()
                })
                .collect()
        };
        if declined.is_empty() {
            return Ok(());
        }
        self.approval_signal.notify_waiters();
        for approval in declined {
            self.append(state, Event::Approval { approval })?;
        }
        Ok(())
    }

    fn approval_map(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, std::collections::BTreeMap<String, Approval>>, AppError>
    {
        self.approvals
            .lock()
            .map_err(|_| AppError::Invalid("approvals are unavailable".into()))
    }

    /// Hands messages the user sent while agy works to the running turn (PreInvocation hook).
    /// Only plain messages are taken; ones with attachments or from jobs wait for the next turn.
    pub fn steer(&self, id: &str) -> Result<Vec<String>, AppError> {
        let cell = self.cell(id)?;
        let mut state = cell
            .state
            .lock()
            .map_err(|_| AppError::Invalid("session unavailable".into()))?;
        Self::active(&state, id)?;
        let taken: Vec<String> = state
            .meta
            .queue
            .iter()
            .filter_map(|item| item.as_str().map(str::to_owned))
            .collect();
        if taken.is_empty() {
            return Ok(vec![]);
        }
        let mut next = state.meta.clone();
        next.queue.retain(|item| !item.is_string());
        self.replace_meta(&mut state, next)?;
        for text in &taken {
            self.append(
                &mut state,
                Event::User {
                    text: text.clone(),
                    kind: "user".into(),
                    attachments: None,
                    steered: Some(true),
                },
            )?;
        }
        let cwd = state.meta.cwd.clone();
        Ok(taken
            .iter()
            .map(|text| self.expand_mentions(text, &cwd))
            .collect())
    }

    /// The message with absolute paths of the files and folders it mentions as `@path`.
    pub(crate) fn expand_mentions(&self, text: &str, cwd: &str) -> String {
        let candidates = antiquota_core::mentions::mention_candidates(text);
        if candidates.is_empty() {
            return text.into();
        }
        let found = self.store.resolve_mentions(cwd, &candidates);
        antiquota_core::mentions::with_references(text, &found)
    }
}
