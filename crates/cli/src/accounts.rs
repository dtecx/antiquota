//! Owns legacy account CLI presentation and isolated adapter composition.
use antiquota_accounts::{FileAccounts, find_agy};
use antiquota_core::AccountStore;
use std::{path::PathBuf, sync::Arc};
pub fn context() -> anyhow::Result<Arc<FileAccounts>> {
    let root = antiquota_store::data_root()?;
    let home = antiquota_store::user_home()?;
    let executable = find_agy(&home).unwrap_or_else(|_| PathBuf::from("agy"));
    Ok(Arc::new(FileAccounts::with_runtime(root, home, executable)))
}
pub fn print(accounts: &FileAccounts) -> anyhow::Result<()> {
    let rows = accounts.accounts()?;
    if rows.is_empty() {
        println!("no signed-in account; run: antiquota login");
        return Ok(());
    }
    let width = rows
        .iter()
        .map(|r| r.slot.len() + 2)
        .max()
        .unwrap_or(10)
        .max(10);
    println!(
        "{:width$} {:30} {:30} LEASE",
        "ACCOUNT", "GEMINI", "CLAUDE/GPT"
    );
    for row in rows {
        let cells: [String; 2] = ["gemini", "cg"].map(|bucket| {
            let wall = row.walls.get(bucket).copied().unwrap_or(0.0);
            if wall > 0.0 {
                format!("walled to {}", local(wall))
            } else if let Some(windows) = row.quota.0.get(bucket) {
                windows
                    .iter()
                    .map(|(window, (fraction, _))| format!("{window} {:.0}%", fraction * 100.0))
                    .collect::<Vec<_>>()
                    .join("  ")
            } else {
                "free, not probed".into()
            }
        });
        println!(
            "{:width$} {:30} {:30} {}",
            row.slot,
            cells[0],
            cells[1],
            if row.busy { "busy" } else { "free" }
        );
    }
    Ok(())
}
pub(crate) fn local(stamp: f64) -> String {
    use chrono::TimeZone;
    chrono::Local
        .timestamp_opt(stamp as i64, 0)
        .single()
        .map(|v| v.format("%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "?".into())
}
pub fn service(accounts: Arc<FileAccounts>) -> anyhow::Result<Arc<antiquota_app::AccountService>> {
    let root = accounts.root.clone();
    Ok(antiquota_app::AccountService::new(
        Arc::new(antiquota_store::FileStore::open(root.clone())?),
        antiquota_store::read_settings(&root)?,
        accounts.clone(),
        Arc::new(antiquota_app::SystemClock),
        Some(accounts),
    ))
}
pub async fn probe(accounts: Arc<FileAccounts>) -> anyhow::Result<()> {
    let service = service(accounts.clone())?;
    for slot in accounts.slots()? {
        let Some(lease) = accounts.lease_named(&slot.name)? else {
            println!("{}: busy, skipped", slot.name);
            continue;
        };
        let result = service
            .probe_account(&*lease, &antiquota_core::Cancellation::default())
            .await;
        let readable = matches!(result, Ok(antiquota_core::ProbeResult::Quota(_)));
        println!(
            "{}: {}",
            slot.name,
            if readable { "ok" } else { "unreadable" }
        );
    }
    Ok(())
}
pub fn stack(accounts: &FileAccounts, reseed: bool) -> anyhow::Result<()> {
    accounts.seed_stack(reseed)?;
    if reseed {
        println!("config re-copied from ~/.gemini (conversations kept)");
    }
    println!("{}", accounts.root.join("stack/.gemini").display());
    Ok(())
}
pub fn unwall(accounts: &FileAccounts, name: &str) -> anyhow::Result<()> {
    for slot in if name == "all" {
        accounts.slots()?.into_iter().map(|s| s.name).collect()
    } else {
        vec![name.into()]
    } {
        for bucket in ["gemini", "cg"] {
            accounts.clear_wall(&slot, bucket)?;
        }
    }
    println!("walls cleared: {name}");
    Ok(())
}
pub async fn login(accounts: Arc<FileAccounts>) -> anyhow::Result<i32> {
    let link = accounts.login_start().await?;
    println!(
        "Sign in with the Google account to add:\n\n  {}\n",
        link.url
    );
    if !antiquota_agy::isolated_fake(&accounts.executable, &accounts.root) {
        antiquota_server::open_url(&link.url);
    }
    println!(
        "Paste the code Google shows after sign-in, then press Enter (agy allows 60 seconds):"
    );
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        use std::io::BufRead;
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    let signal = tokio::signal::ctrl_c();
    tokio::pin!(signal);
    let state = loop {
        let state = accounts.login_state(&link.id)?;
        if state.state != antiquota_core::SignInState::Waiting {
            break state;
        }
        while let Ok(code) = receiver.try_recv() {
            if let Err(error) = accounts.login_submit(&link.id, &code).await {
                eprintln!("{error}");
            }
        }
        tokio::select! {_=&mut signal=>{accounts.login_cancel(&link.id).await?;return Ok(130);},_=tokio::time::sleep(std::time::Duration::from_millis(20))=>{}}
    };
    accounts.login_cancel(&link.id).await?;
    if let Some(error) = state.error {
        eprintln!("antiquota: {error}");
        return Ok(1);
    }
    if state.state == antiquota_core::SignInState::Done {
        println!(
            "{} signed in; it uses antiquota's stack in {}",
            state.name.flatten().unwrap_or_default(),
            accounts.root.join("stack").display()
        );
        Ok(0)
    } else {
        eprintln!("antiquota: sign-in failed");
        Ok(1)
    }
}
pub async fn doctor(accounts: Arc<FileAccounts>) -> anyhow::Result<i32> {
    accounts.seed_stack(false)?;
    // The stack has no private identity. Compare each leased account's loaded metadata.
    println!(
        "antiquota stack in {} (separate from ~/.gemini), as agy reports it:",
        accounts.root.join("stack/.gemini").display()
    );
    println!("sign-in tokens: one file per account (agy file storage), never your OS keyring");
    let linked = accounts.expose_usage()?;
    println!(
        "conversation store link: {}",
        if linked {
            format!(
                "{} -> antiquota",
                accounts.home.join(".config/antigravity").display()
            )
        } else {
            "not set: the path belongs to something else".into()
        }
    );
    let overview = accounts.stack_overview().await?;
    let names = |key: &str| {
        overview[key]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|row| row["here"] == true)
            .filter_map(|row| row["name"].as_str())
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    let expected_skills = names("skills");
    let expected_hooks = names("hooks");
    println!(
        "  skills ({}): {}",
        expected_skills.len(),
        expected_skills.join(", ")
    );
    println!("  hooks:   {}", expected_hooks.join(", "));
    println!("  mcp:     {}", names("mcps").join(", "));
    let mut problems = 0;
    for slot in accounts.slots()? {
        let Some(_lease) = accounts.lease_named(&slot.name)? else {
            println!("{}: busy, skipped", slot.name);
            continue;
        };
        accounts.sync(&slot)?;
        let loaded = accounts.loaded(&slot).await?;
        let identity = accounts.identity(&slot).await?;
        let mut bad = vec![];
        for (key, expected) in [("skills", &expected_skills), ("hooks", &expected_hooks)] {
            if loaded.get(key).and_then(Option::as_ref) != Some(expected) {
                bad.push(format!("agy {key} differ"));
            }
        }
        if identity.as_deref() != Some(&slot.name) {
            bad.push(format!(
                "signs in as {}: its credential store is not its own",
                identity.unwrap_or_else(|| "nobody".into())
            ));
        }
        let private = PathBuf::from(&slot.home).join(antiquota_accounts::AUTH);
        let real = accounts.home.join(antiquota_accounts::AUTH);
        if private.is_symlink()
            || real.exists() && private.canonicalize().ok() == real.canonicalize().ok()
        {
            bad.push("sign-in token must be private".into());
        }
        println!(
            "{}: {}",
            slot.name,
            if bad.is_empty() {
                "ok: signs in as itself, loads the stack's skills and hooks".into()
            } else {
                format!("PROBLEM: {}", bad.join(", "))
            }
        );
        problems += bad.len();
    }
    Ok(i32::from(problems > 0))
}
