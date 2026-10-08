//! Owns private account contexts, exact slot leases and account metadata adapters.
mod farm;
mod files;
mod login;
mod metadata;
mod overview;
mod permissions;
mod stack;
mod stack_edit;
mod stack_import;
use antiquota_core::{
    AccountLease, AccountStore, LoginLink, LoginState, Model, PortError, PortFuture, Quota,
    RoutingEntry, Slot,
};
use antiquota_store::{FileLease, io_error, lock_file};
pub use metadata::{find_agy, parse_quota};
use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
};
pub const AUTH: &str = ".gemini/antigravity-cli/antigravity-oauth-token";
pub const FILE_TOKENS: &str = "127.0.0.1 0 127.0.0.1 0";
#[derive(Clone)]
pub struct FileAccounts {
    pub root: PathBuf,
    pub home: PathBuf,
    pub executable: PathBuf,
    logins: Arc<Mutex<Option<Arc<login::Login>>>>,
    changes: Arc<Mutex<()>>,
    login_gate: Arc<tokio::sync::Mutex<()>>,
}
struct Lease {
    slot: Slot,
    _file: FileLease,
}
impl AccountLease for Lease {
    fn slot(&self) -> &Slot {
        &self.slot
    }
}
pub fn check_name(name: &str) -> Result<(), PortError> {
    antiquota_core::validate_account_name(name)
}
impl FileAccounts {
    pub fn new(root: PathBuf) -> Self {
        Self::with_runtime(
            root,
            antiquota_store::user_home().unwrap_or_default(),
            PathBuf::from("agy"),
        )
    }
    pub fn with_runtime(root: PathBuf, home: PathBuf, executable: PathBuf) -> Self {
        Self {
            root,
            home,
            executable,
            logins: Arc::new(Mutex::new(None)),
            changes: Arc::new(Mutex::new(())),
            login_gate: Arc::new(tokio::sync::Mutex::new(())),
        }
    }
    pub fn slots(&self) -> Result<Vec<Slot>, PortError> {
        let path = self.root.join("slots");
        if !path.exists() {
            return Ok(vec![]);
        }
        if farm::linked(&path) {
            return Err(PortError::Invalid(
                "account directory cannot be linked".into(),
            ));
        }
        let mut slots = vec![];
        for entry in fs::read_dir(path).map_err(io_error)? {
            let entry = entry.map_err(io_error)?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if check_name(&name).is_err() || !entry.file_type().map_err(io_error)?.is_dir() {
                continue;
            }
            let auth = entry.path().join(AUTH);
            if auth.is_file()
                && !farm::linked(&auth)
                && !farm::linked(&entry.path().join(".gemini"))
                && !farm::linked(&entry.path().join(".gemini/antigravity-cli"))
            {
                slots.push(Slot {
                    name,
                    home: entry.path().to_string_lossy().into_owned(),
                });
            }
        }
        slots.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(slots)
    }
    fn lease(&self, slot: Slot) -> Result<Option<Box<dyn AccountLease>>, PortError> {
        lock_file(
            &self
                .root
                .join("state/leases")
                .join(format!("{}.lock", slot.name)),
        )
        .map(|file| file.map(|file| Box::new(Lease { slot, _file: file }) as Box<dyn AccountLease>))
    }
    pub fn sync(&self, slot: &Slot) -> Result<(), PortError> {
        farm::sync(self, slot)
    }
    pub fn seed_stack(&self, force: bool) -> Result<(), PortError> {
        let _guard = self
            .changes
            .lock()
            .map_err(|_| PortError::Unavailable("account changes unavailable".into()))?;
        farm::seed(self, force)
    }
    pub fn expose_usage(&self) -> Result<bool, PortError> {
        farm::expose_usage(self)
    }
    /// Keeps antiquota's own hook entry pointing at this executable.
    pub fn install_hook(&self, program: &std::path::Path) -> Result<bool, PortError> {
        permissions::install_hook(self, program)
    }
}
impl AccountStore for FileAccounts {
    fn accounts(&self) -> Result<Vec<antiquota_core::Account>, PortError> {
        self.accounts_at(files::now())
    }
    fn accounts_at(&self, now: f64) -> Result<Vec<antiquota_core::Account>, PortError> {
        overview::accounts(self, now)
    }
    fn acquire(&self, preferred: Option<&str>) -> Result<Option<Box<dyn AccountLease>>, PortError> {
        let mut slots = self.slots()?;
        if let Some(name) = preferred {
            slots.sort_by_key(|s| s.name != name);
        }
        for slot in slots {
            if self.root.join("state/paused").join(&slot.name).exists() {
                continue;
            }
            if let Some(lease) = self.lease(slot)? {
                return Ok(Some(lease));
            }
        }
        Ok(None)
    }
    fn lease_named(&self, name: &str) -> Result<Option<Box<dyn AccountLease>>, PortError> {
        self.lease(self.slot(name)?)
    }
    fn prepare(&self, slot: &Slot) -> Result<(), PortError> {
        self.sync(slot)
    }
    fn slot(&self, name: &str) -> Result<Slot, PortError> {
        check_name(name)?;
        self.slots()?
            .into_iter()
            .find(|s| s.name == name)
            .ok_or_else(|| PortError::Invalid(format!("no signed-in slot '{name}'")))
    }
    fn pause(&self, name: &str, on: bool) -> Result<(), PortError> {
        files::pause(self, name, on)
    }
    fn remove(&self, name: &str) -> Result<(), PortError> {
        files::remove(self, name)
    }
    fn save_quota(&self, name: &str, quota: &Quota, now: f64) -> Result<(), PortError> {
        files::save_quota(self, name, quota, now)
    }
    fn set_wall(&self, name: &str, bucket: &str, until: f64, why: &str) -> Result<(), PortError> {
        files::set_wall(self, name, bucket, until, why)
    }
    fn clear_wall(&self, name: &str, bucket: &str) -> Result<(), PortError> {
        files::clear_wall(self, name, bucket)
    }
    fn set_verify(&self, name: &str, url: Option<&str>, now: f64) -> Result<(), PortError> {
        files::set_verify(self, name, url, now)
    }
    fn routes(&self, n: usize) -> Result<Vec<RoutingEntry>, PortError> {
        files::routes(self, n)
    }
    fn record_route(&self, entry: &RoutingEntry) -> Result<(), PortError> {
        files::route(self, entry)
    }
    fn cached_models(&self) -> Result<Vec<Model>, PortError> {
        Ok(files::read_json(&self.root.join("state/models.json"))
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default())
    }
    fn save_models(&self, models: &[Model]) -> Result<(), PortError> {
        antiquota_store::write_json_atomic(&self.root.join("state/models.json"), &models)
    }
    fn login_start(&self) -> PortFuture<'_, LoginLink> {
        login::start(self)
    }
    fn login_state(&self, id: &str) -> Result<LoginState, PortError> {
        login::state(self, id)
    }
    fn login_submit<'a>(&'a self, id: &'a str, code: &'a str) -> PortFuture<'a, LoginState> {
        login::submit(self, id, code)
    }
    fn login_cancel<'a>(&'a self, id: &'a str) -> PortFuture<'a, ()> {
        login::cancel(self, id)
    }
    fn cancel_logins(&self) -> PortFuture<'_, ()> {
        login::cancel_all(self)
    }
    fn stack_overview(&self) -> PortFuture<'_, serde_json::Value> {
        Box::pin(stack::overview(self))
    }
    fn stack_info(&self, reseed: bool) -> PortFuture<'_, serde_json::Value> {
        Box::pin(self.stack_info_value(reseed))
    }
    fn stack_file(&self, path: &str) -> Result<serde_json::Value, PortError> {
        self.read_stack_file(path)
    }
    fn write_stack_file(&self, path: &str, text: &str) -> Result<(), PortError> {
        self.write_stack_text(path, text)
    }
    fn stack_item(&self, kind: &str, name: &str) -> Result<serde_json::Value, PortError> {
        self.get_stack_item(kind, name)
    }
    fn put_stack_item(
        &self,
        kind: &str,
        name: &str,
        value: &serde_json::Value,
    ) -> Result<(), PortError> {
        self.put_stack_item(kind, name, value)
    }
    fn delete_stack_item(&self, kind: &str, name: &str) -> Result<(), PortError> {
        self.delete_stack_item(kind, name)
    }
    fn enable_stack_item<'a>(
        &'a self,
        kind: &'a str,
        name: &'a str,
        on: bool,
    ) -> PortFuture<'a, ()> {
        Box::pin(self.enable_stack(kind, name, on))
    }
    fn copy_stack_item(
        &self,
        source: &str,
        kind: &str,
        name: Option<&str>,
    ) -> Result<(), PortError> {
        self.copy_stack(source, kind, name)
    }
    fn permissions(&self) -> Result<antiquota_core::Permissions, PortError> {
        Ok(permissions::read(self))
    }
    fn save_permissions(
        &self,
        allow: Option<Vec<String>>,
        deny: Option<Vec<String>>,
    ) -> Result<serde_json::Value, PortError> {
        permissions::save(self, allow, deny)
    }
    fn add_allow_rule(&self, rule: &str) -> Result<(), PortError> {
        permissions::add_allow(self, rule)
    }
}
#[cfg(test)]
mod tests;
