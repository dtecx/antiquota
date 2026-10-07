//! Owns fake metadata command dispatch without model calls or project edits.
use fake_agy::{FakeError, Script};
use serde_json::json;
use std::{env, fs, path::PathBuf, thread, time::Duration};

pub fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

pub fn handle(args: &[String], script: &Script) -> Result<bool, FakeError> {
    let prompt = flag(args, "-p").unwrap_or_default();
    let command = args.first().map(String::as_str).unwrap_or_default();
    let slot = env::var("ANTIQUOTA_SLOT").unwrap_or_else(|_| {
        PathBuf::from(env::var_os("HOME").unwrap())
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned()
    });
    let metadata = script.metadata_for(&slot);
    if matches!(command, "models" | "--models") || prompt == "/models" {
        if prompt == "/models" {
            println!(
                "{}",
                json!({"command":{"name":"models","data":{"models":metadata.models}}})
            );
        } else {
            for model in &metadata.models {
                println!("{}\t{}", model.id, model.label);
            }
        }
        return Ok(true);
    }
    if prompt == "/quota" {
        let home = PathBuf::from(env::var_os("HOME").unwrap());
        let auth = home.join(".gemini/antigravity-cli/antigravity-oauth-token");
        super::sandbox::isolated(&auth)?;
        let signed_in = auth.exists();
        let identity = if signed_in {
            metadata.identity.as_deref().unwrap_or(&slot).to_string()
        } else {
            super::login::sign_in(&script.login, &auth)?
        };
        super::login::identity_log(args, &identity)?;
        super::descendant::spawn(metadata)?;
        if metadata.quota_delay_ms > 0 {
            thread::sleep(Duration::from_millis(metadata.quota_delay_ms));
        }
        let verification = if signed_in {
            metadata.verify_url.as_ref()
        } else {
            script.login.verify_url.as_ref()
        };
        if let Some(url) = verification {
            println!(
                "{}",
                json!({"status":"ERROR","error":format!("Account eligibility must be verified: {url}")})
            );
            return Err(FakeError::Scenario(
                "account eligibility verification required".into(),
            ));
        }
        if let Some(error) = &metadata.quota_error {
            println!("{}", json!({"status":"ERROR","error":error}));
            return Err(FakeError::Scenario(error.clone()));
        }
        let has_turn = fs::read_to_string(home.join(".fake-agy-turn-count"))
            .ok()
            .and_then(|text| text.parse::<u64>().ok())
            .is_some_and(|turns| turns > 0);
        let spent = env::var("FAKE_AGY_SPENT_SLOT").as_deref() == Ok(slot.as_str());
        let quota = if has_turn {
            metadata
                .quota_after_turn
                .as_ref()
                .or(metadata.quota.as_ref())
        } else {
            metadata.quota.as_ref()
        };
        println!(
            "{}",
            quota
                .cloned()
                .unwrap_or_else(|| fake_agy::metadata::healthy_quota(if spent {
                    0.01
                } else {
                    0.9
                }))
        );
        return Ok(true);
    }
    if matches!(prompt, "/config" | "/hooks" | "/skills") {
        let name = prompt.trim_start_matches('/');
        let data = match name {
            "skills" => {
                json!({"skills": metadata.skills.iter().map(|name| json!({"name":name})).collect::<Vec<_>>()})
            }
            "hooks" => {
                json!({"hooks": metadata.hooks.iter().map(|name| json!({"name":name})).collect::<Vec<_>>()})
            }
            _ => json!({"config":metadata.config}),
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
