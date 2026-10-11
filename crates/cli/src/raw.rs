//! Owns legacy one-call routing, unchanged argv and exit-code accounting.
use antiquota_accounts::FileAccounts;
use antiquota_core::{
    AccountStore, Cancellation, Clock, Failure, Mode, TurnRequest, bucket_of_args, classify_error,
    error_text, rank_accounts,
};
use serde_json::json;
use std::sync::Arc;
pub async fn run(accounts: Arc<FileAccounts>, mut args: Vec<String>) -> anyhow::Result<i32> {
    let pin = if args.first().is_some_and(|v| v == "--slot") && args.len() > 1 {
        let value = args.remove(1);
        args.remove(0);
        Some(value)
    } else {
        None
    };
    if args.first().is_some_and(|v| v == "--") {
        args.remove(0);
    }
    let rows = accounts.accounts()?;
    if rows.is_empty() {
        anyhow::bail!("antiquota: no signed-in account; run: antiquota login");
    }
    if pin
        .as_ref()
        .is_some_and(|pin| !rows.iter().any(|row| &row.slot == pin))
    {
        anyhow::bail!(
            "antiquota: no signed-in slot '{}'",
            pin.as_deref().unwrap_or("")
        );
    }
    let bucket = bucket_of_args(&args);
    let settings = antiquota_store::read_settings(&accounts.root)?;
    let cancel = Cancellation::default();
    let clock = antiquota_app::SystemClock;
    let lease = loop {
        let rows = accounts.accounts()?;
        let ranked = if let Some(pin) = &pin {
            rows.iter()
                .filter(|r| {
                    &r.slot == pin && r.walls.get(bucket).copied().unwrap_or(0.0) <= clock.now()
                })
                .map(|r| r.slot.clone())
                .collect()
        } else {
            rank_accounts(
                &rows,
                bucket,
                None,
                &settings,
                &accounts.routes(400)?,
                clock.now(),
            )
        };
        if ranked.is_empty() {
            let reset = rows
                .iter()
                .filter(|r| pin.as_ref().is_none_or(|pin| &r.slot == pin))
                .map(|r| r.walls.get(bucket).copied().unwrap_or(0.0))
                .min_by(f64::total_cmp)
                .unwrap_or(0.0);
            eprintln!(
                "antiquota: every account is out of {} quota; next reset {}",
                if bucket == "cg" {
                    "Claude/GPT"
                } else {
                    "Gemini"
                },
                crate::accounts::local(reset)
            );
            return Ok(75);
        }
        let mut selected = None;
        for name in ranked {
            if let Some(lease) = accounts.share_named(&name)? {
                selected = Some(lease);
                break;
            }
        }
        if let Some(lease) = selected {
            break lease;
        }
        clock.wait(1.0, cancel.clone()).await?;
    };
    accounts.prepare(lease.slot())?;
    let runner = antiquota_agy::AgyRunner {
        executable: accounts.executable.clone(),
        hook_program: std::env::current_exe()?,
        isolated_fake: antiquota_agy::isolated_fake(&accounts.executable, &accounts.root),
    };
    let request = TurnRequest {
        session: String::new(),
        turn: 0,
        prompt: String::new(),
        model: antiquota_core::DEFAULT_MODEL.into(),
        cwd: std::env::current_dir()?.to_string_lossy().into_owned(),
        mode: Mode::Default,
        conversation_id: None,
        slot: lease.slot().clone(),
        state_dir: accounts.root.join("state").to_string_lossy().into_owned(),
        add_dirs: vec![],
    };
    let started = clock.now();
    let mut result = runner.raw(&request, &args).await?;
    let error = error_text(result.code, &result.out, &result.err);
    let walled = result
        .decoded
        .as_ref()
        .map(|r| r.status == "quota")
        .unwrap_or_else(|| classify_error(&error) == Some(Failure::Quota));
    if result
        .decoded
        .as_ref()
        .is_some_and(|r| r.status == "SUCCESS")
    {
        result.code = 0;
    }
    let name = lease.slot().name.clone();
    if walled {
        crate::accounts::service(accounts.clone())?
            .quota_failure(lease, bucket, &cancel)
            .await?;
    }
    if result.code != 0 {
        antiquota_store::write_atomic(
            &accounts
                .root
                .join("state/failures")
                .join(format!("{}-{}.log", started.floor(), name)),
            format!(
                "rc={}\n--- stdout\n{}\n--- stderr\n{}\n",
                result.code, result.out, result.err
            )
            .as_bytes(),
        )?;
    }
    antiquota_store::append_json(
        &accounts.root.join("state/calls.jsonl"),
        &json!({"t":started.floor(),"slot":name,"bucket":bucket,"rc":result.code,"secs":((clock.now()-started)*10.0).round()/10.0,"walled":walled}),
    )?;
    Ok(if walled { 75 } else { result.code })
}
