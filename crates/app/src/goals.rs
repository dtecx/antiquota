//! Owns goal mode: setting, resuming and clearing goals, and what follows a finished turn —
//! goal nudges, the check command, the independent verifier and denied-command follow-ups.
//! The decisions themselves live in `antiquota_features::goal`.
use crate::{AppError, Service};
use antiquota_core::{
    AccountLease, AskRequest, Cancellation, Event, Goal, Session, bucket_of_model,
    goal::{goal_prompt, parse_verdict, resume_prompt, verifier_prompt},
    rank_accounts,
};
use antiquota_features::goal::{self as rules, AfterTurn, CheckResult, Verification};
use serde_json::{Value, json};
use std::sync::Arc;

const CHECK_TIMEOUT: f64 = 1800.0;
const CHECK_KEEP: usize = 3000;

/// What a finished turn left behind for the follow-up decision.
#[derive(Debug, Clone, Default)]
pub struct TurnOutcome {
    pub ok: bool,
    pub reply: String,
    pub denied: Vec<String>,
}

fn last_chars(text: &str, count: usize) -> String {
    let total = text.chars().count();
    text.chars().skip(total.saturating_sub(count)).collect()
}

impl Service {
    /// Sets a goal (round 0, active) and, unless `start` is false, sends its prompt.
    #[allow(clippy::too_many_arguments)]
    pub fn set_goal(
        self: &Arc<Self>,
        id: &str,
        objective: &str,
        check: &str,
        verifier: bool,
        verifier_model: &str,
        max_rounds: u64,
        start: bool,
    ) -> Result<Session, AppError> {
        let objective = objective.trim();
        if objective.is_empty() {
            return Err(AppError::Invalid("empty objective".into()));
        }
        let goal = Goal {
            objective: objective.into(),
            check: check.trim().into(),
            verifier,
            verifier_model: verifier_model.into(),
            max_rounds,
            round: 0,
            state: "active".into(),
            extra: Default::default(),
        };
        self.update_goal(id, |slot| *slot = Some(goal.clone()))?;
        self.notice(id, "goal", &format!("Goal set: {objective}"), json!({}))?;
        if start {
            self.send(id, goal_prompt(&goal), vec![], "goal".into())?;
        }
        Ok(self.get(id)?.0)
    }

    /// Restarts a goal from round 0 with a nudge.
    pub fn resume_goal(self: &Arc<Self>, id: &str) -> Result<(), AppError> {
        let mut resumed = None;
        self.update_goal(id, |slot| {
            if let Some(goal) = slot {
                goal.state = "active".into();
                goal.round = 0;
                resumed = Some(goal.clone());
            }
        })?;
        let goal = resumed.ok_or_else(|| AppError::Invalid("no goal".into()))?;
        self.send(id, resume_prompt(&goal), vec![], "nudge".into())?;
        Ok(())
    }

    pub fn clear_goal(&self, id: &str) -> Result<(), AppError> {
        self.update_goal(id, |slot| *slot = None)
    }

    pub(crate) fn goal_active(&self, id: &str) -> Result<bool, AppError> {
        Ok(self
            .get(id)?
            .0
            .goal
            .is_some_and(|goal| goal.state == "active"))
    }

    fn update_goal(
        &self,
        id: &str,
        change: impl FnOnce(&mut Option<Goal>),
    ) -> Result<(), AppError> {
        let cell = self.cell(id)?;
        let mut state = cell
            .state
            .lock()
            .map_err(|_| AppError::Invalid("session unavailable".into()))?;
        Self::active(&state, id)?;
        let mut next = state.meta.clone();
        change(&mut next.goal);
        if next.goal != state.meta.goal {
            self.replace_meta(&mut state, next)?;
        }
        Ok(())
    }

    pub(crate) fn set_status(
        &self,
        id: &str,
        status: &str,
        until: Option<f64>,
    ) -> Result<(), AppError> {
        let cell = self.cell(id)?;
        let mut state = cell
            .state
            .lock()
            .map_err(|_| AppError::Invalid("session unavailable".into()))?;
        if state.meta.status == status && state.meta.status_until == until {
            return Ok(());
        }
        let mut next = state.meta.clone();
        next.status = status.into();
        next.status_until = until;
        self.replace_meta(&mut state, next)
    }

    /// The final reply and denied commands of turn `turn`; records a denial notice.
    pub(crate) fn turn_outcome(
        &self,
        id: &str,
        turn: u64,
        ok: bool,
    ) -> Result<TurnOutcome, AppError> {
        let (_, records) = self.get(id)?;
        let mut outcome = TurnOutcome {
            ok,
            ..Default::default()
        };
        for record in &records {
            match &record.event {
                Event::Assistant { turn: t, text, .. } if *t == turn => {
                    outcome.reply = text.clone();
                }
                Event::Tool {
                    turn: t,
                    name,
                    params,
                    state,
                    error: Some(error),
                    ..
                } if *t == turn && name == "run_command" && state == "error" => {
                    let message = error
                        .get("message")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                        .unwrap_or_else(|| {
                            error
                                .as_str()
                                .map_or_else(|| error.to_string(), str::to_owned)
                        });
                    if message.contains("denied permission")
                        && let Some(line) = params.get("CommandLine").and_then(Value::as_str)
                        && !line.is_empty()
                    {
                        outcome.denied.push(line.into());
                    }
                }
                _ => {}
            }
        }
        if !outcome.denied.is_empty() {
            self.notice(
                id,
                "denied",
                "agy denied a tool call that headless mode cannot ask about.",
                json!({"actions": [], "commands": outcome.denied}),
            )?;
        }
        Ok(outcome)
    }

    /// The next prompt (and its kind) after a turn: a goal nudge, a retry, or a bounded
    /// follow-up after denied commands. `None` lets queued messages run.
    pub(crate) async fn follow_up(
        self: &Arc<Self>,
        id: &str,
        outcome: &TurnOutcome,
        errors: &mut u32,
        continues: &mut u32,
    ) -> Result<Option<(String, String)>, AppError> {
        let (session, cancel) = {
            let cell = self.cell(id)?;
            let state = cell
                .state
                .lock()
                .map_err(|_| AppError::Invalid("session unavailable".into()))?;
            (state.meta.clone(), state.cancel.clone())
        };
        if cancel.is_cancelled() {
            return Ok(None);
        }
        let Some(mut goal) = session.goal.clone().filter(|g| g.state == "active") else {
            let allowance = self
                .defaults()?
                .auto_continue
                .parse::<f64>()
                .unwrap_or(2.0)
                .max(0.0) as u32;
            return Ok(if outcome.ok {
                rules::denied_follow_up(&outcome.denied, continues, allowance)
                    .map(|text| (text, "nudge".into()))
            } else {
                None
            });
        };
        let waiting: Vec<String> = session
            .jobs
            .iter()
            .filter(|job| job["notify"] == true && !crate::http_truthy(&job["ended"]))
            .map(|job| job["cmd"].as_str().unwrap_or("").to_owned())
            .collect();
        let decision = rules::after_turn(&mut goal, outcome.ok, &outcome.reply, &waiting, errors);
        let saved = goal.clone();
        self.update_goal(id, |slot| *slot = Some(saved))?;
        let nudge =
            |prompt: String| Some((rules::with_denials(prompt, &outcome.denied), "nudge".into()));
        Ok(match decision {
            AfterTurn::Done => None,
            AfterTurn::WaitForJobs { notice } | AfterTurn::Abandon { notice } => {
                self.notice(id, "goal", &notice, json!({}))?;
                None
            }
            AfterTurn::Nudge { notice, prompt } => {
                if let Some(notice) = notice {
                    self.notice(id, "goal", &notice, json!({}))?;
                }
                nudge(prompt)
            }
            AfterTurn::Retry {
                delay,
                notice,
                prompt,
            } => {
                self.notice(id, "goal", &notice, json!({}))?;
                self.clock.wait(delay, cancel.clone()).await?;
                (!cancel.is_cancelled()).then(|| (prompt, "nudge".into()))
            }
            AfterTurn::Evaluate => self
                .evaluate_goal(id, goal, &session, &cancel)
                .await?
                .and_then(nudge),
        })
    }

    async fn evaluate_goal(
        self: &Arc<Self>,
        id: &str,
        mut goal: Goal,
        session: &Session,
        cancel: &Cancellation,
    ) -> Result<Option<String>, AppError> {
        let check = if goal.check.is_empty() {
            None
        } else {
            self.set_status(id, "checking", None)?;
            let outcome = self
                .shell
                .run(&session.cwd, &goal.check, CHECK_TIMEOUT, cancel.clone())
                .await;
            if cancel.is_cancelled() {
                return Ok(None);
            }
            let (code, output) = match outcome {
                Ok(outcome) => (outcome.code, last_chars(&outcome.output, CHECK_KEEP)),
                Err(error) => (None, error.to_string()),
            };
            let result = CheckResult {
                command: goal.check.clone(),
                code,
                output,
            };
            self.notice(
                id,
                "check",
                &format!("Check `{}` exited {}.", result.command, result.exit_text()),
                json!({
                    "rc": code.map_or_else(|| json!("timeout"), |c| json!(c)),
                    "output": result.output,
                    "passed": result.passed(),
                }),
            )?;
            Some(result)
        };
        let verification = if rules::needs_verifier(&goal, check.as_ref()) {
            self.set_status(id, "verifying", None)?;
            let verification = self.verify(id, &goal, session, cancel).await?;
            if cancel.is_cancelled() {
                return Ok(None);
            }
            verification
        } else {
            Verification::Skipped
        };
        let next = rules::evaluate(&mut goal, check.as_ref(), &verification);
        let saved = goal.clone();
        self.update_goal(id, |slot| *slot = Some(saved))?;
        self.set_status(id, "running", None)?;
        if next.is_none() {
            self.notice(id, "goal", "Goal met.", json!({"met": true}))?;
        }
        Ok(next)
    }

    /// Asks a fresh, read-only agy conversation whether the objective is met: the verifier's
    /// model first, then the session's model when no account has quota for the former.
    async fn verify(
        self: &Arc<Self>,
        id: &str,
        goal: &Goal,
        session: &Session,
        cancel: &Cancellation,
    ) -> Result<Verification, AppError> {
        let mut models: Vec<String> = vec![];
        for model in [&goal.verifier_model, &session.model] {
            if !model.is_empty() && !models.contains(model) {
                models.push(model.clone());
            }
        }
        for (index, model) in models.iter().enumerate() {
            let bucket = bucket_of_model(model);
            while !cancel.is_cancelled() {
                let Some(lease) = self.verifier_lease(bucket, cancel).await? else {
                    break;
                };
                let slot = lease.slot().clone();
                let result = self
                    .runner
                    .ask(
                        AskRequest {
                            prompt: verifier_prompt(&goal.objective),
                            model: model.clone(),
                            cwd: session.cwd.clone(),
                            slot: slot.clone(),
                            add_dirs: vec![],
                            timeout: 1800.0,
                        },
                        cancel.clone(),
                    )
                    .await;
                match result {
                    Ok(answer) if answer.status == "quota" => {
                        self.quota_failure(&*lease, bucket, cancel).await?;
                        drop(lease);
                        self.notice(
                            id,
                            "info",
                            &format!(
                                "The verifier hit the {} quota on {}.",
                                pool_name(bucket),
                                slot.name
                            ),
                            json!({}),
                        )?;
                    }
                    Ok(answer) if answer.status == "stopped" => {
                        return Ok(Verification::Unavailable);
                    }
                    Ok(answer) => return self.verdict(id, &answer.response, &slot.name, model),
                    Err(error) => {
                        tracing::warn!(%error, "verifier call failed");
                        return self.verdict(id, "", &slot.name, model);
                    }
                }
            }
            if let Some(next) = models.get(index + 1)
                && !cancel.is_cancelled()
            {
                self.notice(
                    id,
                    "info",
                    &format!(
                        "No account has {} quota for the verifier ({model}); {next} verifies instead.",
                        pool_name(bucket)
                    ),
                    json!({}),
                )?;
            }
        }
        if !cancel.is_cancelled() {
            self.notice(
                id,
                "verdict",
                "No account has quota left for the verifier.",
                json!({"done": null}),
            )?;
        }
        Ok(Verification::Unavailable)
    }

    fn verdict(
        &self,
        id: &str,
        reply: &str,
        slot: &str,
        model: &str,
    ) -> Result<Verification, AppError> {
        let Some(verdict) = parse_verdict(reply) else {
            let shown = if reply.is_empty() {
                "(empty)".to_owned()
            } else {
                last_chars(reply, 600)
            };
            self.notice(
                id,
                "verdict",
                &format!("The verifier gave no verdict. Its reply: {shown}"),
                json!({"done": null}),
            )?;
            return Ok(Verification::Unavailable);
        };
        self.notice(
            id,
            "verdict",
            &verdict.reason,
            json!({"done": verdict.done, "missing": verdict.missing, "slot": slot, "model": model}),
        )?;
        Ok(Verification::Verdict(verdict))
    }

    /// An idle account with live quota in `bucket`, or `None` when none has any.
    async fn verifier_lease(
        &self,
        bucket: &str,
        cancel: &Cancellation,
    ) -> Result<Option<Box<dyn AccountLease>>, AppError> {
        loop {
            if cancel.is_cancelled() {
                return Ok(None);
            }
            let ranked = rank_accounts(
                &self.account_overview()?,
                bucket,
                None,
                &self.defaults()?,
                &self.accounts.routes(1000)?,
                self.clock.now(),
            );
            if ranked.is_empty() {
                return Ok(None);
            }
            for name in &ranked {
                if let Some(lease) = self.accounts.lease_named(name)? {
                    return Ok(Some(lease));
                }
            }
            self.clock.wait(1.0, cancel.clone()).await?;
        }
    }
}

fn pool_name(bucket: &str) -> &'static str {
    if bucket == "cg" {
        "Claude / GPT"
    } else {
        "Gemini"
    }
}
