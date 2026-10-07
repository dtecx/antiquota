//! Owns synthetic OAuth code input and identity logs inside disposable account homes.
use fake_agy::{FakeError, metadata::Login};
use serde_json::json;
use std::{
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

pub fn sign_in(login: &Login, auth: &Path) -> Result<String, FakeError> {
    eprintln!("Authentication required. Please visit the URL to log in:");
    eprintln!("{}", login.url);
    eprintln!("Or, paste the authorization code here and press Enter:");
    io::stderr().flush()?;
    let mut code = String::new();
    if let Some(delay) = login.auto_complete_ms {
        std::thread::sleep(std::time::Duration::from_millis(delay));
        code = login.code.clone();
    } else {
        io::stdin().read_line(&mut code)?;
    }
    if code.trim() != login.code || login.error.is_some() {
        eprintln!("Error: authentication failed");
        return Err(FakeError::Scenario(
            login
                .error
                .clone()
                .unwrap_or_else(|| "Error: authentication failed".into()),
        ));
    }
    let email = env::var("FAKE_AGY_EMAIL").unwrap_or_else(|_| login.email.clone());
    if !email
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b"._%+@-".contains(&b))
        || email.len() > 100
        || email.starts_with('.')
        || !email.contains('@')
    {
        return Err(FakeError::Scenario("invalid synthetic identity".into()));
    }
    fs::create_dir_all(auth.parent().unwrap())?;
    fs::write(auth, json!({"access_token":"synthetic-access-token","refresh_token":"synthetic-refresh-token","token_type":"Bearer","expiry_date":4070908800000_u64}).to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(auth, fs::Permissions::from_mode(0o600))?;
    }
    fs::write(auth.parent().unwrap().join("jetski_state.pbtxt"), "")?;
    Ok(email)
}

pub fn identity_log(args: &[String], email: &str) -> Result<(), FakeError> {
    if let Some(path) = super::commands::flag(args, "--log-file") {
        let path = PathBuf::from(path);
        super::sandbox::isolated(&path)?;
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(
            path,
            format!("I1006 OAuth: authenticated successfully as {email}\n"),
        )?;
    }
    Ok(())
}
