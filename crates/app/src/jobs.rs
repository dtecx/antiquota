//! Owns background jobs, interval loops, daily schedules and the scheduler tick that keeps
//! the computer awake and dispatches due loops.
use crate::{AppError, Service};
use antiquota_core::{
    Cancellation, Event, Verdict,
    jobs::{
        JOB_TAIL, JOBS_KEPT, Job, LOG_VIEW, LOOP_MIN, background_requests, check_every, clean_tail,
        job_check_in, job_result, parse_hhmm,
    },
    policy,
};
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};

/// How often the scheduler looks at loops, keep-awake and expired background approvals.
const TICK: Duration = Duration::from_secs(5);
/// How often a running job's check-in interval is evaluated.
const WATCH: Duration = Duration::from_secs(5);

impl Service {
    /// Starts `command` detached in the session folder, logging to the session's job folder.
    pub fn job_start(
        self: &Arc<Self>,
        id: &str,
        command: &str,
        every: u64,
        notify: bool,
        by: &str,
    ) -> Result<Job, AppError> {
        let command = command.trim();
        if command.is_empty() {
            return Err(AppError::Invalid("empty command".into()));
        }
        let cell = self.cell(id)?;
        let mut state = cell
            .state
            .lock()
            .map_err(|_| AppError::Invalid("session unavailable".into()))?;
        Self::active(&state, id)?;
        let jid = uuid::Uuid::new_v4().simple().to_string()[..8].to_owned();
        let log = self.store.job_log_path(id, &jid);
        let pid = self.jobs.start(&state.meta.cwd, command, &log)?;
        let job = Job {
            id: jid,
            cmd: command.into(),
            pid: Some(pid),
            log,
            started: self.clock.now(),
            ended: None,
            code: None,
            every,
            notify,
            by: by.into(),
            stopped: false,
            extra: Default::default(),
        };
        let mut next = state.meta.clone();
        next.jobs.push(job.to_value());
        let excess = next.jobs.len().saturating_sub(JOBS_KEPT);
        next.jobs.drain(..excess);
        self.replace_meta(&mut state, next)?;
        self.append(
            &mut state,
            Event::Job {
                phase: "started".into(),
                job: job.to_value(),
            },
        )?;
        drop(state);
        let service = self.clone();
        let (sid, watched) = (id.to_owned(), job.clone());
        tokio::spawn(async move { service.watch_job(sid, watched).await });
        Ok(job)
    }

    /// Waits for a job to end, sending progress check-ins while the session is idle, then
    /// records the end and, unless the job was stopped or is silent, sends agy its result.
    async fn watch_job(self: Arc<Self>, id: String, job: Job) {
        let Some(pid) = job.pid else { return };
        let jobs = self.jobs.clone();
        let mut ended = Box::pin(async move { jobs.wait(pid).await });
        let mut last = self.clock.now();
        let code = loop {
            tokio::select! {
                code = &mut ended => break code.unwrap_or(None),
                _ = tokio::time::sleep(WATCH) => {}
            }
            let now = self.clock.now();
            if job.every > 0 && now - last >= job.every as f64 {
                last = now;
                if job.notify && self.idle_and_running(&id) {
                    let tail = self.log_tail(&job.log, JOB_TAIL);
                    let _ = self.send(&id, job_check_in(&job, now, &tail), vec![], "job".into());
                }
            }
        };
        let Ok(Some(job)) = self.finish_job(&id, &job.id, code) else {
            return;
        };
        if job.notify && !job.stopped && !self.stopped(&id) {
            let tail = self.log_tail(&job.log, JOB_TAIL);
            let _ = self.send(&id, job_result(&job, &tail), vec![], "job".into());
        }
    }

    fn finish_job(&self, id: &str, jid: &str, code: Option<i32>) -> Result<Option<Job>, AppError> {
        let cell = self.cell(id)?;
        let mut state = cell
            .state
            .lock()
            .map_err(|_| AppError::Invalid("session unavailable".into()))?;
        if state.deleting {
            return Ok(None);
        }
        let mut next = state.meta.clone();
        let Some(slot) = next
            .jobs
            .iter_mut()
            .find(|j| j.get("id").and_then(Value::as_str) == Some(jid))
        else {
            return Ok(None);
        };
        let Some(mut job) = Job::from_value(slot) else {
            return Ok(None);
        };
        job.ended = Some(self.clock.now());
        job.code = code;
        *slot = job.to_value();
        self.replace_meta(&mut state, next)?;
        self.append(
            &mut state,
            Event::Job {
                phase: "ended".into(),
                job: job.to_value(),
            },
        )?;
        Ok(Some(job))
    }

    /// Stops a running job's process tree; stopping an ended job changes nothing.
    pub fn job_stop(&self, id: &str, jid: &str) -> Result<Job, AppError> {
        let cell = self.cell(id)?;
        let mut state = cell
            .state
            .lock()
            .map_err(|_| AppError::Invalid("session unavailable".into()))?;
        Self::active(&state, id)?;
        let mut next = state.meta.clone();
        let slot = next
            .jobs
            .iter_mut()
            .find(|j| j.get("id").and_then(Value::as_str) == Some(jid))
            .ok_or_else(|| AppError::Missing(jid.into()))?;
        let mut job = Job::from_value(slot).ok_or_else(|| AppError::Missing(jid.into()))?;
        if !job.running() {
            return Ok(job);
        }
        job.stopped = true;
        *slot = job.to_value();
        self.replace_meta(&mut state, next)?;
        if let Some(pid) = job.pid {
            self.jobs.stop(pid)?;
        }
        Ok(job)
    }

    /// The job and the end of its log, without terminal colour codes.
    pub fn job_log(&self, id: &str, jid: &str) -> Result<(Job, String), AppError> {
        let job = self
            .get(id)?
            .0
            .jobs
            .iter()
            .filter_map(Job::from_value)
            .find(|j| j.id == jid)
            .ok_or_else(|| AppError::Missing(jid.into()))?;
        let text = self.log_tail(&job.log, LOG_VIEW);
        Ok((job, text))
    }

    fn log_tail(&self, log: &str, chars: usize) -> String {
        let raw = self.jobs.read_log(log, chars * 4).unwrap_or_default();
        clean_tail(&raw, chars)
    }

    fn stopped(&self, id: &str) -> bool {
        self.cell(id)
            .ok()
            .and_then(|c| c.state.lock().ok().map(|s| s.cancel.is_cancelled()))
            .unwrap_or(true)
    }

    fn idle_and_running(&self, id: &str) -> bool {
        self.cell(id)
            .ok()
            .and_then(|c| {
                c.state
                    .lock()
                    .ok()
                    .map(|s| !s.busy && !s.cancel.is_cancelled() && !s.deleting)
            })
            .unwrap_or(false)
    }

    /// Jobs agy asked for at the end of a successful reply. They pass the same permission
    /// policy as its tool calls: allowed ones start, denied ones are reported, and the rest
    /// wait for the user's answer on an approval card.
    pub(crate) fn jobs_from_reply(self: &Arc<Self>, id: &str, reply: &str) -> Result<(), AppError> {
        let requests = background_requests(reply);
        if requests.is_empty() {
            return Ok(());
        }
        let every = check_every(reply);
        let permissions = self.accounts.permissions()?;
        let session = self.get(id)?.0;
        for command in requests {
            let running = session
                .jobs
                .iter()
                .filter_map(Job::from_value)
                .any(|j| j.running() && j.cmd == command);
            if running {
                continue;
            }
            let decision = policy::decide(
                session.mode,
                "run_command",
                &json!({ "CommandLine": command }),
                &permissions,
            );
            match decision.verdict {
                Verdict::Allow => {
                    self.job_start(id, &command, every, true, "agent")?;
                }
                Verdict::Deny => self.notice(
                    id,
                    "denied",
                    &format!("A deny rule in antiquota's permissions blocks the background job `{command}`."),
                    json!({"actions": [], "commands": [command]}),
                )?,
                Verdict::Ask => {
                    self.request_background(id, &command, every, decision.rule)?;
                }
            }
        }
        Ok(())
    }

    /// Sets an interval loop (`every` seconds, at least a minute) or a daily schedule (`at`,
    /// local "HH:MM"), or removes it when both are empty. Returns the daily schedule's next run.
    pub fn set_loop(
        &self,
        id: &str,
        every: u64,
        prompt: &str,
        at: Option<&str>,
    ) -> Result<Option<f64>, AppError> {
        let at = at.map(str::trim).filter(|a| !a.is_empty());
        let prompt = prompt.trim();
        if (every > 0 || at.is_some()) && prompt.is_empty() {
            return Err(AppError::Invalid(
                "what should it send? /loop 10m <prompt> or /schedule 03:00 <prompt>".into(),
            ));
        }
        let now = self.clock.now();
        let (config, next) = match at {
            Some(at) => {
                let (hour, minute) = parse_hhmm(at)
                    .ok_or_else(|| AppError::Invalid("time like 03:00 or 18:30".into()))?;
                let next = self.clock.next_daily(hour, minute, now);
                (
                    Some(json!({"every": 86400, "at": at, "prompt": prompt, "next": next})),
                    Some(next),
                )
            }
            None if every > 0 => (
                Some(json!({"every": every.max(LOOP_MIN), "prompt": prompt, "next": now})),
                None,
            ),
            None => (None, None),
        };
        let cell = self.cell(id)?;
        let mut state = cell
            .state
            .lock()
            .map_err(|_| AppError::Invalid("session unavailable".into()))?;
        Self::active(&state, id)?;
        if config.is_some() && !state.busy {
            // Installing a loop resumes a stopped session's scheduled work.
            state.cancel = Cancellation::default();
        }
        let mut next_meta = state.meta.clone();
        next_meta.loop_config = config;
        self.replace_meta(&mut state, next_meta)?;
        Ok(next)
    }

    /// Running background jobs across sessions.
    pub fn running_jobs(&self) -> usize {
        self.sessions()
            .unwrap_or_default()
            .iter()
            .flat_map(|s| s.jobs.iter())
            .filter_map(Job::from_value)
            .filter(Job::running)
            .count()
    }

    /// Watches jobs an earlier engine started and runs the scheduler every few seconds.
    pub fn start_scheduler(self: &Arc<Self>) {
        for session in self.sessions().unwrap_or_default() {
            for job in session.jobs.iter().filter_map(Job::from_value) {
                if job.running() {
                    let service = self.clone();
                    let id = session.id.clone();
                    tokio::spawn(async move { service.watch_job(id, job).await });
                }
            }
        }
        let service = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(TICK).await;
                if service.background_cancel.is_cancelled() {
                    break;
                }
                service.scheduler_tick();
            }
        });
    }

    /// One scheduler pass: keep-awake, expired background approvals, and due loops.
    pub fn scheduler_tick(self: &Arc<Self>) {
        let keep = self
            .defaults()
            .map(|s| s.keep_awake != "off")
            .unwrap_or(true);
        if keep && self.busy_count() > 0 {
            self.awake.hold();
        } else {
            self.awake.release();
        }
        self.expire_background_approvals();
        let now = self.clock.now();
        for session in self.sessions().unwrap_or_default() {
            let Some(config) = session.loop_config.clone() else {
                continue;
            };
            let due = config["next"].as_f64().is_some_and(|next| now >= next);
            if !due || !self.idle_and_running(&session.id) {
                continue;
            }
            let next = match config["at"].as_str().and_then(parse_hhmm) {
                Some((hour, minute)) => self.clock.next_daily(hour, minute, now),
                None => now + config["every"].as_f64().unwrap_or(LOOP_MIN as f64),
            };
            let mut advanced = config.clone();
            advanced["next"] = json!(next);
            if self.store_loop(&session.id, advanced).is_err() {
                continue;
            }
            let prompt = config["prompt"].as_str().unwrap_or("").to_owned();
            if let Err(error) = self.send(&session.id, prompt, vec![], "loop".into()) {
                tracing::warn!(%error, "loop prompt failed");
            }
        }
    }

    fn store_loop(&self, id: &str, config: Value) -> Result<(), AppError> {
        let cell = self.cell(id)?;
        let mut state = cell
            .state
            .lock()
            .map_err(|_| AppError::Invalid("session unavailable".into()))?;
        let mut next = state.meta.clone();
        next.loop_config = Some(config);
        self.replace_meta(&mut state, next)
    }
}
