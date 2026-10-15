//! Owns one agy OAuth conversation, cancellation and lease-safe token adoption.
use crate::{
    AUTH, FILE_TOKENS, FileAccounts, check_name, files,
    metadata::{google_url, identity_from_log, parse_quota},
};
use antiquota_core::{
    AccountStore, LoginLink, LoginState, PARKED_SECONDS, PortError, PortFuture, SignInState,
};
use antiquota_pty::Console;
use antiquota_store::{io_error, lock_file, write_atomic};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
pub struct Login {
    id: String,
    temp: PathBuf,
    console: Arc<Console>,
    state: Mutex<LoginState>,
    cancelled: AtomicBool,
}
fn locked() -> PortError {
    PortError::Unavailable("sign-in state unavailable".into())
}
fn active(accounts: &FileAccounts, id: &str) -> Result<Option<Arc<Login>>, PortError> {
    Ok(accounts
        .logins
        .lock()
        .map_err(|_| locked())?
        .as_ref()
        .filter(|l| l.id == id)
        .cloned())
}
pub fn state(accounts: &FileAccounts, id: &str) -> Result<LoginState, PortError> {
    match active(accounts, id)? {
        Some(login) => Ok(login.state.lock().map_err(|_| locked())?.clone()),
        None => Ok(LoginState::replaced()),
    }
}
pub fn start(accounts: &FileAccounts) -> PortFuture<'_, LoginLink> {
    Box::pin(async move {
        let _gate = accounts.login_gate.lock().await;
        let old = accounts.logins.lock().map_err(|_| locked())?.take();
        if let Some(old) = old {
            stop(
                &old,
                "replaced by a newer sign-in link (only the newest one works)",
            )
            .await?;
        }
        let slots = accounts.root.join("slots");
        if crate::farm::linked(&slots) {
            return Err(PortError::Invalid(
                "account directory cannot be linked".into(),
            ));
        }
        fs::create_dir_all(&slots).map_err(io_error)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&slots, fs::Permissions::from_mode(0o700)).map_err(io_error)?;
        }
        let id = uuid::Uuid::new_v4().simple().to_string()[..16].to_owned();
        let temp = slots.join(format!(".login-{id}"));
        fs::create_dir(&temp).map_err(io_error)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&temp, fs::Permissions::from_mode(0o700)).map_err(io_error)?;
        }
        let log = temp.join("signin.log");
        let environment: BTreeMap<_, _> = [
            ("HOME".into(), temp.to_string_lossy().into_owned()),
            ("USERPROFILE".into(), temp.to_string_lossy().into_owned()),
            ("SSH_CONNECTION".into(), FILE_TOKENS.into()),
        ]
        .into();
        let args = vec![
            "-p".into(),
            "/quota".into(),
            "--output-format".into(),
            "json".into(),
            "--model".into(),
            antiquota_core::DEFAULT_MODEL.into(),
            "--log-file".into(),
            log.to_string_lossy().into_owned(),
        ];
        let executable = accounts.executable.clone();
        let folder = temp.clone();
        let result = tokio::task::spawn_blocking(move || {
            Console::spawn(&executable, &args, &folder, &environment)
        })
        .await
        .map_err(|_| locked())?;
        let console = match result {
            Ok(console) => console,
            Err(error) => {
                let _ = fs::remove_dir_all(&temp);
                return Err(error);
            }
        };
        let login = Arc::new(Login {
            id: id.clone(),
            temp,
            console,
            state: Mutex::new(LoginState::waiting()),
            cancelled: AtomicBool::new(false),
        });
        *accounts.logins.lock().map_err(|_| locked())? = Some(login.clone());
        let adapter = accounts.clone();
        let worker = login.clone();
        tokio::spawn(async move {
            finish(adapter, worker).await;
        });
        let url = tokio::time::timeout(Duration::from_secs(20), async {
            loop {
                if let Some(url) = google_url(&clean_output(&login.console.text()), false) {
                    break Ok::<_, PortError>(url);
                }
                let current = login.state.lock().map_err(|_| locked())?.clone();
                if current.state != SignInState::Waiting {
                    break Err(PortError::Invalid(
                        current
                            .error
                            .unwrap_or_else(|| "agy printed no sign-in URL".into()),
                    ));
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await;
        match url {
            Ok(Ok(url)) => Ok(LoginLink {
                id,
                url,
                seconds: 60,
            }),
            Ok(Err(error)) => Err(error),
            Err(_) => {
                stop(&login, "agy printed no sign-in URL").await?;
                Err(PortError::Invalid("agy printed no sign-in URL".into()))
            }
        }
    })
}
pub fn submit<'a>(
    accounts: &'a FileAccounts,
    id: &'a str,
    code: &'a str,
) -> PortFuture<'a, LoginState> {
    Box::pin(async move {
        let code = code.trim();
        if code.is_empty() || code.chars().count() > 2000 || code.contains(['\r', '\n', '\0']) {
            return Err(PortError::Invalid(
                "paste the single-line code Google showed after sign-in".into(),
            ));
        }
        let Some(login) = active(accounts, id)? else {
            return Err(PortError::Invalid(
                "this sign-in was replaced by a newer link; use the newest one".into(),
            ));
        };
        {
            let state = login.state.lock().map_err(|_| locked())?;
            if state.state != SignInState::Waiting || login.cancelled.load(Ordering::SeqCst) {
                return Err(PortError::Invalid(
                    state
                        .error
                        .clone()
                        .unwrap_or_else(|| "this sign-in already finished".into()),
                ));
            }
            // ConPTY reads Enter as a carriage return; a bare newline never ends agy's line.
            let enter = if cfg!(windows) { "\r" } else { "\n" };
            login.console.write(&format!("{code}{enter}"))?;
        }
        let _ = tokio::time::timeout(Duration::from_secs(90), async {
            loop {
                if login.state.lock().map_err(|_| locked())?.state != SignInState::Waiting {
                    break Ok::<_, PortError>(());
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await;
        state(accounts, id)
    })
}
pub fn cancel<'a>(accounts: &'a FileAccounts, id: &'a str) -> PortFuture<'a, ()> {
    Box::pin(async move {
        let login = {
            let mut current = accounts.logins.lock().map_err(|_| locked())?;
            if current.as_ref().is_some_and(|l| l.id == id) {
                current.take()
            } else {
                None
            }
        };
        if let Some(login) = login {
            stop(&login, "cancelled").await?;
        }
        Ok(())
    })
}
async fn stop(login: &Arc<Login>, reason: &str) -> Result<(), PortError> {
    {
        let mut state = login.state.lock().map_err(|_| locked())?;
        login.cancelled.store(true, Ordering::SeqCst);
        if state.state == SignInState::Waiting {
            state.state = SignInState::Failed;
            state.error = Some(reason.into());
        }
    }
    let console = login.console.clone();
    tokio::task::spawn_blocking(move || {
        console.stop();
        console.reap();
    })
    .await
    .map_err(|_| locked())?;
    // Windows can hold a just-ended process's folder for a moment, so retry briefly.
    for _ in 0..50 {
        if fs::remove_dir_all(&login.temp).is_ok() || !login.temp.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(40)).await;
    }
    Ok(())
}
async fn finish(accounts: FileAccounts, login: Arc<Login>) {
    let result: Result<(), PortError> = async {
        let until = tokio::time::Instant::now() + Duration::from_secs(600);
        while !login.console.finished()? && !login.cancelled.load(Ordering::SeqCst) {
            if tokio::time::Instant::now() > until {
                stop(&login, "sign-in timed out").await?;
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        for _ in 0..25 {
            if login.console.drained() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        if login.cancelled.load(Ordering::SeqCst) {
            return Ok(());
        }
        let output = clean_output(&login.console.text());
        let value = output
            .find('{')
            .zip(output.rfind('}'))
            .and_then(|(start, end)| serde_json::from_str::<Value>(&output[start..=end]).ok())
            .unwrap_or_default();
        let name = fs::read_to_string(login.temp.join("signin.log"))
            .ok()
            .and_then(|text| identity_from_log(&text));
        let verify = value["error"]
            .as_str()
            .and_then(|error| google_url(error, true));
        let token = login.temp.join(AUTH);
        let has_token = token.is_file() && !token.is_symlink();
        let success = value["status"] == "SUCCESS" && has_token;
        let retained = name.is_some() && has_token && verify.is_some();
        let mut current = login.state.lock().map_err(|_| locked())?;
        if login.cancelled.load(Ordering::SeqCst) || current.state != SignInState::Waiting {
            return Ok(());
        }
        if success || retained {
            let name = name.unwrap_or_else(|| format!("account-{}", files::now().floor()));
            check_name(&name)?;
            adopt(&accounts, &login, &name)?;
            for bucket in ["gemini", "cg"] {
                accounts.clear_wall(&name, bucket)?;
            }
            accounts.set_verify(&name, None, files::now())?;
            if retained {
                let url = verify.clone().unwrap_or_default();
                accounts.set_verify(&name, Some(&url), files::now())?;
                for bucket in ["gemini", "cg"] {
                    accounts.set_wall(
                        &name,
                        bucket,
                        files::now() + PARKED_SECONDS,
                        "verify the account with Google",
                    )?;
                }
                current.state = SignInState::Verify;
                current.error = value["error"]
                    .as_str()
                    .map(|s| s.lines().next().unwrap_or("").into());
            } else {
                current.state = SignInState::Done;
                if let Ok(quota) = parse_quota(&value) {
                    accounts.save_quota(&name, &quota, files::now())?;
                }
            }
            current.name = Some(Some(name));
            current.verify_url = Some(verify);
        } else {
            current.state = SignInState::Failed;
            current.error = Some(
                output
                    .lines()
                    .rfind(|line| line.trim().to_lowercase().starts_with("error"))
                    .unwrap_or("the sign-in ended before it finished")
                    .trim()
                    .into(),
            );
            let reason = current.error.as_deref().unwrap_or("sign-in failed");
            write_atomic(
                &accounts
                    .root
                    .join("state/failures")
                    .join(format!("signin-{}.log", login.id)),
                format!("error={reason}\n").as_bytes(),
            )?;
            let _ = fs::remove_dir_all(&login.temp);
        }
        Ok(())
    }
    .await;
    if let Err(error) = result {
        tracing::warn!(%error,"sign-in did not complete");
        if let Ok(mut state) = login.state.lock() {
            state.state = SignInState::Failed;
            state.error = Some(error.to_string());
        }
        login.console.stop();
        let _ = fs::remove_dir_all(&login.temp);
    }
}
fn adopt(accounts: &FileAccounts, login: &Login, name: &str) -> Result<(), PortError> {
    let Some(_lease) = lock_file(
        &accounts
            .root
            .join("state/leases")
            .join(format!("{name}.lock")),
    )?
    else {
        return Err(PortError::Invalid(format!(
            "'{name}' is running a call; stop it first"
        )));
    };
    let target = accounts.root.join("slots").join(name);
    if crate::farm::linked(&target) {
        return Err(PortError::Invalid("account HOME cannot be linked".into()));
    }
    for rel in [".gemini", ".gemini/antigravity-cli"] {
        if crate::farm::linked(&target.join(rel)) {
            return Err(PortError::Invalid(
                "private token parent cannot be linked".into(),
            ));
        }
    }
    fs::create_dir_all(target.join(".gemini/antigravity-cli")).map_err(io_error)?;
    for rel in [AUTH, ".gemini/antigravity-cli/jetski_state.pbtxt"] {
        let source = login.temp.join(rel);
        if source.is_file() {
            if source.is_symlink() || crate::farm::linked(&target.join(rel)) {
                return Err(PortError::Invalid("private token cannot be linked".into()));
            }
            let bytes = fs::read(&source).map_err(io_error)?;
            write_atomic(&target.join(rel), &bytes)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(target.join(rel), fs::Permissions::from_mode(0o600))
                    .map_err(io_error)?;
            }
            fs::remove_file(source).map_err(io_error)?;
        }
    }
    let leftover = target.join(".antiquota-login");
    if leftover.exists() {
        fs::remove_dir_all(&leftover).map_err(io_error)?;
    }
    fs::rename(&login.temp, &leftover).map_err(io_error)?;
    accounts.sync(&antiquota_core::Slot {
        name: name.into(),
        home: target.to_string_lossy().into_owned(),
    })
}

pub fn cancel_all(accounts: &FileAccounts) -> PortFuture<'_, ()> {
    Box::pin(async move {
        let login = accounts.logins.lock().map_err(|_| locked())?.take();
        if let Some(login) = login {
            stop(&login, "cancelled").await?;
        }
        Ok(())
    })
}

fn clean_output(text: &str) -> String {
    regex::regex!(r"\x1b\[[0-9;?]*[ -/]*[@-~]|\x1b\][^\x07]*\x07")
        .replace_all(text, "")
        .replace("\r\n", "\n")
        .replace('\r', "\n")
}
