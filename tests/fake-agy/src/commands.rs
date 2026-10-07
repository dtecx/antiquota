//! Zero-cost metadata, sign-in, verifier, and dictation responses for fake agy.
use fake_agy::{FakeError, Script};
use serde_json::{Value, json};
use std::{
    env, fs,
    io::{self, Write},
    path::PathBuf,
};

pub fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

pub fn handle(args: &[String], script: &Script) -> Result<bool, FakeError> {
    let prompt = flag(args, "-p").unwrap_or_default();
    let command = args.first().map(String::as_str).unwrap_or_default();
    if matches!(command, "models" | "--models") || prompt == "/models" {
        if prompt == "/models" {
            println!(
                "{}",
                json!({"command":{"name":"models","data":{"models":[{"id":"gemini-3.8-flash-high","label":"Gemini 3.8 Flash High"}]}}})
            );
        } else {
            println!("gemini-3.8-flash-high\tGemini 3.8 Flash High");
        }
        return Ok(true);
    }
    if prompt == "/quota" {
        let home = PathBuf::from(env::var_os("HOME").unwrap());
        let auth = home.join(".gemini/antigravity-cli/antigravity-oauth-token");
        super::sandbox::isolated(&auth)?;
        if !auth.exists() {
            eprintln!("Authentication required. Please visit the URL to log in:");
            eprintln!("https://accounts.google.com/o/oauth2/auth?client_id=fixture&state=fixture");
            eprintln!("Or, paste the authorization code here and press Enter:");
            io::stderr().flush()?;
            let mut code = String::new();
            io::stdin().read_line(&mut code)?;
            if code.trim() != "good-code" {
                return Err(FakeError::Scenario("synthetic sign-in rejected".into()));
            }
            fs::create_dir_all(auth.parent().unwrap())?;
            fs::write(&auth, "synthetic-account-token")?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&auth, fs::Permissions::from_mode(0o600))?;
            }
            fs::write(auth.parent().unwrap().join("jetski_state.pbtxt"), "")?;
            if let Some(path) = flag(args, "--log-file") {
                let path = PathBuf::from(path);
                super::sandbox::isolated(&path)?;
                let email = env::var("FAKE_AGY_EMAIL").unwrap_or_else(|_| "new@example.com".into());
                fs::write(
                    path,
                    format!("I1006 OAuth: authenticated successfully as {email}\n"),
                )?;
            }
        }
        let spent = env::var("FAKE_AGY_SPENT_SLOT").ok() == env::var("ANTIQUOTA_SLOT").ok()
            && env::var_os("FAKE_AGY_SPENT_SLOT").is_some();
        let groups:Vec<_>=["Gemini","Claude/GPT"].iter().map(|name|json!({"name":name,"buckets":[
            {"window":"5h","remaining_fraction":if spent && *name=="Gemini" {0.01} else {0.9},"reset_time":"2099-01-01T05:00:00Z"},
            {"window":"weekly","remaining_fraction":0.9,"reset_time":"2099-01-07T00:00:00Z"}]})).collect();
        println!(
            "{}",
            json!({"status":"SUCCESS","command":{"name":"quota","data":{"groups":groups}}})
        );
        return Ok(true);
    }
    if matches!(prompt, "/config" | "/hooks" | "/skills") {
        let name = prompt.trim_start_matches('/');
        let data: Value = if name == "skills" {
            json!({"skills":[]})
        } else {
            json!({name:{}})
        };
        println!(
            "{}",
            json!({"status":"SUCCESS","command":{"name":name,"data":data}})
        );
        return Ok(true);
    }
    if prompt.contains("VERDICT:") {
        println!(
            "{}",
            json!({"status":"SUCCESS","response":if script.verdict_done {"All good.\nVERDICT: DONE"} else {"- tests are missing\nVERDICT: NOT_DONE"},"usage":{"input_tokens":100,"output_tokens":10}})
        );
        return Ok(true);
    }
    if prompt.starts_with("Listen to the audio file ") {
        println!(
            "{}",
            json!({"status":"SUCCESS","response":env::var("FAKE_AGY_TRANSCRIPT").unwrap_or_else(|_|"NO_SPEECH".into())})
        );
        return Ok(true);
    }
    Ok(false)
}
