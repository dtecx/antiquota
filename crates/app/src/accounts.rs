//! Owns account probes, cooldown persistence and account event publication through ports.
use crate::{AccountService, AppError};
use antiquota_core::{
    Account, AccountLease, COOLDOWNS, Cancellation, Event, EventRecord, FALLBACK_WALL, Failure,
    PARKED_SECONDS, ProbeResult, RoutingEntry, Settings, StreamEvent, cooldown_strikes,
    quota_fresh, quota_walls,
};
use std::sync::Arc;
impl AccountService {
    pub fn defaults(&self) -> Result<Settings, AppError> {
        Ok(self
            .store
            .settings()?
            .unwrap_or_else(|| self.settings.clone()))
    }
    pub fn public_defaults(&self) -> Result<Settings, AppError> {
        let mut settings = self.defaults()?;
        settings.extra.clear();
        Ok(settings)
    }
    pub fn account_overview(&self) -> Result<Vec<Account>, AppError> {
        Ok(self.accounts.accounts_at(self.clock.now())?)
    }
    pub fn announce_accounts(&self) -> Result<Vec<Account>, AppError> {
        let accounts = self.account_overview()?;
        let _ = self.events.send(StreamEvent {
            sid: None,
            record: EventRecord::new(
                self.clock.now(),
                Event::Accounts {
                    accounts: accounts.clone(),
                },
            ),
        });
        Ok(accounts)
    }
    pub fn pause_account(&self, name: &str, on: bool) -> Result<Vec<Account>, AppError> {
        self.require_account(name)?;
        self.accounts.pause(name, on)?;
        self.route(RoutingEntry {
            event: if on { "paused" } else { "resumed" }.into(),
            slot: Some(name.into()),
            why: Some("by the user".into()),
            ..Default::default()
        })?;
        self.announce_accounts()
    }
    pub fn remove_account(&self, name: &str) -> Result<(), AppError> {
        self.require_account(name)?;
        self.accounts.remove(name)?;
        self.announce_accounts()?;
        Ok(())
    }
    pub fn unwall(&self, name: &str) -> Result<(), AppError> {
        let accounts = self.account_overview()?;
        for account in accounts.iter().filter(|a| name == "all" || a.slot == name) {
            for bucket in ["gemini", "cg"] {
                self.accounts.clear_wall(&account.slot, bucket)?;
            }
        }
        self.announce_accounts()?;
        Ok(())
    }
    fn require_account(&self, name: &str) -> Result<(), AppError> {
        if self.account_overview()?.iter().any(|a| a.slot == name) {
            Ok(())
        } else {
            Err(AppError::Missing(name.into()))
        }
    }
    pub async fn check_account(&self, name: &str) -> Result<Vec<Account>, AppError> {
        self.require_account(name)?;
        let Some(lease) = self.accounts.lease_named(name)? else {
            return Err(AppError::Invalid(format!(
                "{name} is running a call; it is checked when that ends"
            )));
        };
        self.probe_account(&*lease, &self.background_cancel).await?;
        drop(lease);
        self.announce_accounts()
    }
    /// Probes idle accounts whose reading is at least max_age old; returns those with a new reading.
    pub async fn refresh_accounts(&self, max_age: f64) -> Result<Vec<String>, AppError> {
        let mut updated = vec![];
        for account in self.account_overview()? {
            if self.background_cancel.is_cancelled() {
                break;
            }
            if self.clock.now() - account.probed.unwrap_or(0.0) < max_age {
                continue;
            }
            let Some(lease) = self.accounts.lease_named(&account.slot)? else {
                continue;
            };
            if matches!(
                self.probe_account(&*lease, &self.background_cancel).await?,
                ProbeResult::Quota(_)
            ) {
                updated.push(account.slot);
            }
        }
        self.refresh_models().await?;
        self.announce_accounts()?;
        Ok(updated)
    }
    async fn refresh_models(&self) -> Result<(), AppError> {
        if self.models_fresh.load(std::sync::atomic::Ordering::SeqCst) {
            return Ok(());
        }
        let Some(probe) = &self.quota else {
            return Ok(());
        };
        let Some(first) = self.account_overview()?.first().cloned() else {
            return Ok(());
        };
        let Some(lease) = self.accounts.lease_named(&first.slot)? else {
            return Ok(());
        };
        self.accounts.prepare(lease.slot())?;
        let future = probe.models(lease.slot());
        tokio::pin!(future);
        let result = loop {
            tokio::select! {result=&mut future=>break result,_=tokio::time::sleep(std::time::Duration::from_millis(20))=>{if self.background_cancel.is_cancelled(){return Ok(())}}}
        };
        if let Ok(models) = result
            && !models.is_empty()
        {
            self.accounts.save_models(&models)?;
            self.models_fresh
                .store(true, std::sync::atomic::Ordering::SeqCst);
        }
        Ok(())
    }
    pub fn start_account_polling(self: &Arc<Self>) {
        let service = self.clone();
        tokio::spawn(async move {
            while !service.background_cancel.is_cancelled() {
                let freshness = service.defaults().map(|s| quota_fresh(&s)).unwrap_or(600.0);
                if let Err(error) = service.refresh_accounts(freshness).await {
                    tracing::warn!(%error,"account refresh failed");
                }
                if let Err(error) = service
                    .clock
                    .wait(freshness, service.background_cancel.clone())
                    .await
                {
                    tracing::warn!(%error,"account refresh wait failed");
                    break;
                }
            }
        });
    }
    pub async fn probe_account(
        &self,
        lease: &dyn AccountLease,
        cancel: &Cancellation,
    ) -> Result<ProbeResult, AppError> {
        let Some(probe) = &self.quota else {
            return Ok(ProbeResult::Unreadable);
        };
        self.accounts.prepare(lease.slot())?;
        let future = probe.probe(lease.slot());
        tokio::pin!(future);
        let result = loop {
            tokio::select! {result=&mut future=>break result?,_=tokio::time::sleep(std::time::Duration::from_millis(20))=>{if cancel.is_cancelled() || self.background_cancel.is_cancelled(){return Ok(ProbeResult::Unreadable)}}}
        };
        if cancel.is_cancelled() || self.background_cancel.is_cancelled() {
            return Ok(ProbeResult::Unreadable);
        }
        let name = &lease.slot().name;
        let now = self.clock.now();
        match &result {
            ProbeResult::Quota(quota) => {
                self.accounts.save_quota(name, quota, now)?;
                self.accounts.set_verify(name, None, now)?;
                for (bucket, until) in quota_walls(quota, now) {
                    if let Some(until) = until {
                        self.accounts
                            .set_wall(name, &bucket, until, "quota panel")?;
                    } else {
                        self.accounts.clear_wall(name, &bucket)?;
                    }
                }
            }
            ProbeResult::Verify(url) => {
                self.accounts.set_verify(name, Some(url), now)?;
                self.park_account(name, "verify the account with Google")?;
            }
            ProbeResult::SignedOut => {
                self.park_account(name, "signed out: sign in again")?;
                self.route(RoutingEntry {
                    event: "signed-out".into(),
                    slot: Some(name.clone()),
                    why: Some("agy rejected the account's token".into()),
                    ..Default::default()
                })?;
            }
            ProbeResult::Unreadable => {}
        }
        Ok(result)
    }
    pub(crate) fn park_account(&self, name: &str, why: &str) -> Result<(), AppError> {
        for bucket in ["gemini", "cg"] {
            self.accounts
                .set_wall(name, bucket, self.clock.now() + PARKED_SECONDS, why)?;
        }
        Ok(())
    }
    pub(crate) fn route(&self, mut entry: RoutingEntry) -> Result<(), AppError> {
        entry.ts = self.clock.now();
        self.accounts.record_route(&entry)?;
        Ok(())
    }
    pub async fn quota_failure(
        &self,
        lease: &dyn AccountLease,
        bucket: &str,
        cancel: &Cancellation,
    ) -> Result<Failure, AppError> {
        let panel = self.probe_account(lease, cancel).await?;
        if cancel.is_cancelled() || self.background_cancel.is_cancelled() {
            return Ok(Failure::Quota);
        }
        let name = &lease.slot().name;
        let now = self.clock.now();
        let until = self
            .account_overview()?
            .iter()
            .find(|a| &a.slot == name)
            .and_then(|a| a.walls.get(bucket).copied())
            .unwrap_or(0.0);
        if until > now {
            self.route(RoutingEntry {
                event: "wall".into(),
                slot: Some(name.clone()),
                bucket: Some(bucket.into()),
                until: Some(until),
                why: Some("quota spent".into()),
                ..Default::default()
            })?;
        } else if matches!(panel, ProbeResult::Unreadable) {
            let until = now + FALLBACK_WALL;
            self.accounts.set_wall(name, bucket, until, "quota error")?;
            self.route(RoutingEntry {
                event: "wall".into(),
                slot: Some(name.clone()),
                bucket: Some(bucket.into()),
                until: Some(until),
                why: Some("quota error, panel unreadable".into()),
                ..Default::default()
            })?;
        } else {
            let strikes = cooldown_strikes(&self.accounts.routes(400)?, name, bucket, now);
            let rest = COOLDOWNS[strikes.min(2)];
            self.accounts.set_wall(
                name,
                bucket,
                now + rest,
                &format!("rate limited ({}x)", strikes + 1),
            )?;
            self.route(RoutingEntry {
                event: "cooldown".into(),
                slot: Some(name.clone()),
                bucket: Some(bucket.into()),
                secs: Some(rest),
                why: Some("rate limited, quota left".into()),
                ..Default::default()
            })?;
        }
        self.announce_accounts()?;
        Ok(Failure::Quota)
    }
}
