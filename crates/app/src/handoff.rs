//! Owns leased account acquisition and bounded feature-driven attempt continuation.
use crate::{AppError, Service};
use antiquota_core::{
    AFFINITY_WAIT, AccountLease, COOLDOWNS, Cancellation, Command, Event, RoutingEntry, Turn,
    bucket_of_model, next_reset, quota_fresh, rank_accounts,
};
use antiquota_features::{Feature, handoff::Handoff};
use serde_json::json;
use std::sync::Arc;
const CONTINUE: &str = "Continue exactly where you stopped. Your previous attempt was cut off by an account quota limit; do not redo work that is already finished.";
impl Service {
    pub(crate) async fn one_turn(
        self: &Arc<Self>,
        id: &str,
        original: &str,
    ) -> Result<crate::goals::TurnOutcome, AppError> {
        let cell = self.cell(id)?;
        let (cancel, model) = {
            let state = cell
                .state
                .lock()
                .map_err(|_| AppError::Invalid("session unavailable".into()))?;
            (state.cancel.clone(), state.meta.model.clone())
        };
        if cancel.is_cancelled() {
            return Ok(Default::default());
        }
        let count = self.account_overview()?.len();
        if count == 0 {
            self.notice(
                id,
                "noaccount",
                "No Google account is signed in to antiquota yet. Add one, then send again.",
                json!({}),
            )?;
            return Err(AppError::Invalid("no available account".into()));
        }
        let bucket = bucket_of_model(&model);
        let mut prompt = original.to_owned();
        let mut retries = 0;
        let mut failovers = 0;
        while failovers <= 2 * count + 1 && !cancel.is_cancelled() {
            let preferred = self.get(id)?.0.last_slot;
            let Some(lease) = self
                .acquire_account(id, bucket, preferred.as_deref(), &cancel)
                .await?
            else {
                return Ok(Default::default());
            };
            if cancel.is_cancelled() {
                return Ok(Default::default());
            }
            if preferred.as_deref() != Some(lease.slot().name.as_str()) {
                let mut entry = RoutingEntry {
                    event: "pick".into(),
                    session: Some(id.into()),
                    slot: Some(lease.slot().name.clone()),
                    bucket: Some(bucket.into()),
                    previous: preferred,
                    ..Default::default()
                };
                if entry.previous.is_none() {
                    entry.extra.insert("previous".into(), json!(null));
                }
                self.route(entry)?;
            }
            let result = self.attempt(id, &prompt, &*lease, &cancel).await?;
            if cancel.is_cancelled() {
                return Ok(Default::default());
            }
            let name = &lease.slot().name;
            if result.status == "quota" {
                self.quota_failure(&*lease, bucket, &cancel).await?;
            } else if result.status == "auth" {
                self.park_account(name, "signed out: sign in again")?;
                self.route(RoutingEntry {
                    event: "signed-out".into(),
                    slot: Some(name.clone()),
                    why: Some("agy rejected the account's token".into()),
                    ..Default::default()
                })?;
            } else if result.status == "SUCCESS" {
                let stale = self
                    .account_overview()?
                    .iter()
                    .find(|a| &a.slot == name)
                    .is_some_and(|a| {
                        self.clock.now() - a.probed.unwrap_or(0.0)
                            > quota_fresh(&self.defaults().unwrap_or_default())
                    });
                if stale {
                    self.probe_account(&*lease, &cancel).await?;
                }
            }
            if cancel.is_cancelled() {
                return Ok(Default::default());
            }
            let meta = self.get(id)?.0;
            let turn = Turn {
                session: id.into(),
                number: meta.turns,
                slot: name.clone(),
                model: model.clone(),
                status: result.status.clone(),
                retries,
                failovers: failovers as u32,
            };
            let continuation = Handoff.after_turn(&meta, &turn)?.into_iter().next();
            let Some(Command::ContinueTurn { delay, fresh, .. }) = continuation else {
                drop(lease);
                self.announce_accounts()?;
                return self.turn_outcome(id, meta.turns, result.status == "SUCCESS");
            };
            let again = if meta.conversation_id.is_some() {
                CONTINUE.into()
            } else {
                prompt.clone()
            };
            match result.status.as_str() {
                "quota" => {
                    self.notice(id,"handoff",&format!("{name} reached its quota or rate limit. Passing the turn to the next account."),json!({"slot":name}))?;
                    failovers += 1;
                    prompt = again;
                }
                "auth" => {
                    self.notice(id,"handoff",&format!("Google rejected {name}'s sign-in: sign it in again. Passing the turn to the next account."),json!({"slot":name}))?;
                    failovers += 1;
                    prompt = again;
                }
                "transient" if delay > 0.0 => {
                    let error = tail(&result.error, 200);
                    self.notice(
                        id,
                        "info",
                        &format!(
                            "agy hit a temporary error on {name}; retrying in {delay}s. ({error})"
                        ),
                        json!({}),
                    )?;
                    self.route(RoutingEntry {
                        event: "retry".into(),
                        slot: Some(name.clone()),
                        bucket: Some(bucket.into()),
                        session: Some(id.into()),
                        wait: Some(delay),
                        why: Some(error),
                        ..Default::default()
                    })?;
                    retries += 1;
                    prompt = again;
                }
                "transient" => {
                    self.accounts.set_wall(
                        name,
                        bucket,
                        self.clock.now() + COOLDOWNS[1],
                        "repeated errors",
                    )?;
                    self.route(RoutingEntry {
                        event: "cooldown".into(),
                        slot: Some(name.clone()),
                        bucket: Some(bucket.into()),
                        secs: Some(COOLDOWNS[1]),
                        why: Some("repeated errors".into()),
                        ..Default::default()
                    })?;
                    self.notice(id,"handoff",&format!("{name} keeps failing; it rests 5 min. Passing the turn to the next account."),json!({"slot":name}))?;
                    retries = 0;
                    failovers += 1;
                    prompt = again;
                }
                "conversation_lost" if fresh => {
                    self.notice(id,"handoff","The conversation could not be resumed on this account; starting a fresh one with a handoff summary.",json!({}))?;
                    prompt = self.fresh_handoff(id, &prompt)?;
                    failovers += 1;
                }
                _ => return Ok(Default::default()),
            }
            drop(lease);
            self.announce_accounts()?;
            if delay > 0.0 {
                self.clock.wait(delay, cancel.clone()).await?;
            }
        }
        Ok(Default::default())
    }
    async fn acquire_account(
        &self,
        id: &str,
        bucket: &str,
        preferred: Option<&str>,
        cancel: &Cancellation,
    ) -> Result<Option<Box<dyn AccountLease>>, AppError> {
        {
            let cell = self.cell(id)?;
            let mut state = cell
                .state
                .lock()
                .map_err(|_| AppError::Invalid("session unavailable".into()))?;
            if state.meta.status != "waiting" {
                let mut next = state.meta.clone();
                next.status = "waiting".into();
                next.status_until = None;
                self.replace_meta(&mut state, next)?;
            }
        }
        let started = self.clock.now();
        loop {
            if cancel.is_cancelled() || self.background_cancel.is_cancelled() {
                return Ok(None);
            }
            let accounts = self.account_overview()?;
            let settings = self.defaults()?;
            let ranked = rank_accounts(
                &accounts,
                bucket,
                preferred,
                &settings,
                &self.accounts.routes(1000)?,
                self.clock.now(),
            );
            if ranked.is_empty() {
                let pool = if bucket == "cg" {
                    "Claude / GPT"
                } else {
                    "Gemini"
                };
                let held: Vec<_> = accounts
                    .iter()
                    .filter(|a| {
                        a.paused && a.walls.get(bucket).copied().unwrap_or(0.0) <= self.clock.now()
                    })
                    .map(|a| a.slot.as_str())
                    .collect();
                let reset = next_reset(&accounts, bucket, self.clock.now());
                if held.is_empty() && self.goal_active(id)? {
                    self.notice(
                        id,
                        "wall",
                        &format!(
                            "Every account is out of {pool} quota. Sleeping until {}, then the goal continues.",
                            self.clock.local_hhmm(reset)
                        ),
                        json!({"reset":reset}),
                    )?;
                    self.set_status(id, "sleeping", (reset > 0.0).then_some(reset))?;
                    let pause = if reset > 0.0 {
                        (reset - self.clock.now() + 30.0).max(5.0)
                    } else {
                        60.0
                    };
                    self.clock.wait(pause, cancel.clone()).await?;
                    self.set_status(id, "waiting", None)?;
                    continue;
                }
                let text = if !held.is_empty() {
                    format!(
                        "Every account with {pool} quota is paused ({}). Resume one in the Accounts panel, then send again.",
                        held.join(", ")
                    )
                } else {
                    format!(
                        "Every account is out of {pool} quota. Send again later, or use goal mode to wait automatically."
                    )
                };
                self.notice(id, "wall", &text, json!({"reset":reset}))?;
                return Ok(None);
            }
            let affinity =
                preferred.is_some_and(|name| ranked.first().is_some_and(|first| first == name));
            let waiting_for_affinity = affinity && self.clock.now() - started < AFFINITY_WAIT;
            for name in ranked.iter().take(if waiting_for_affinity {
                1
            } else {
                ranked.len()
            }) {
                if let Some(lease) = self.accounts.lease_named(name)? {
                    if affinity && preferred != Some(name.as_str()) {
                        self.route(RoutingEntry {
                            event: "affinity-broke".into(),
                            slot: Some(name.clone()),
                            bucket: Some(bucket.into()),
                            session: Some(id.into()),
                            previous: preferred.map(str::to_owned),
                            why: Some("previous account busy".into()),
                            ..Default::default()
                        })?;
                    }
                    return Ok(Some(lease));
                }
            }
            self.clock.wait(1.0, cancel.clone()).await?;
        }
    }
    fn fresh_handoff(&self, id: &str, prompt: &str) -> Result<String, AppError> {
        let cell = self.cell(id)?;
        let mut state = cell
            .state
            .lock()
            .map_err(|_| AppError::Invalid("session unavailable".into()))?;
        let mut next = state.meta.clone();
        next.conversation_id = None;
        self.replace_meta(&mut state, next)?;
        let mut summary = format!(
            "You are taking over work from an earlier agent session whose conversation could not be resumed. Its handoff state follows; check the current files yourself before relying on it.\n\n# Handoff: {}\nFolder: {}\n",
            state.meta.title, state.meta.cwd
        );
        for record in state
            .records
            .iter()
            .rev()
            .filter(|r| {
                matches!(
                    r.event,
                    Event::User { .. } | Event::Assistant { .. } | Event::Tool { .. }
                )
            })
            .take(24)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            match &record.event {
                Event::User { text, .. } => {
                    summary.push_str(&format!("Request: {}\n", tail(text, 2000)))
                }
                Event::Assistant { text, .. } => {
                    summary.push_str(&format!("Reply: {}\n", tail(text, 4000)))
                }
                Event::Tool { name, output, .. } => summary.push_str(&format!(
                    "Tool {name}: {}\n",
                    tail(output.as_deref().unwrap_or(""), 1000)
                )),
                _ => {}
            }
        }
        summary.push_str(&format!("\nContinue. Current request:\n{prompt}"));
        Ok(summary)
    }
    pub(crate) fn notice(
        &self,
        id: &str,
        kind: &str,
        text: &str,
        data: serde_json::Value,
    ) -> Result<(), AppError> {
        self.streamed(
            id,
            Event::Notice {
                kind: kind.into(),
                text: text.into(),
                data,
            },
        )?;
        Ok(())
    }
}
fn tail(text: &str, count: usize) -> String {
    text.chars()
        .rev()
        .take(count)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}
