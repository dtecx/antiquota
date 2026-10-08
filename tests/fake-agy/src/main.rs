//! Play agy protocol fixtures in an explicitly isolated test HOME/data directory.
mod commands;
mod descendant;
mod hook;
mod login;
mod sandbox;
use fake_agy::{FakeError, Scenario, Script, TurnContext, protocol};
use serde_json::json;
use std::{
    env,
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    thread,
    time::Duration,
};

fn run() -> Result<i32, FakeError> {
    sandbox::check()?;
    let args: Vec<String> = env::args().skip(1).collect();
    sandbox::record(&args)?;
    if descendant::handle(&args)? {
        return Ok(0);
    }
    let mut script = if let Some(path) = env::var_os("FAKE_AGY_SCRIPT") {
        let path = PathBuf::from(path);
        sandbox::isolated(&path)?;
        serde_json::from_slice::<Script>(&fs::read(path)?)?
    } else {
        Script::default()
    };
    if let Ok(value) = env::var("FAKE_AGY_SCENARIO") {
        script.scenario = serde_json::from_value(json!(value))?;
    }
    if let Ok(value) = env::var("STUB_SLEEP") {
        script.delay_ms = (value
            .parse::<f64>()
            .map_err(|_| FakeError::Scenario("STUB_SLEEP must be numeric".into()))?
            * 1000.0) as u64;
    }
    if commands::handle(&args, &script)? {
        return Ok(0);
    }
    let cwd = env::current_dir()?;
    sandbox::isolated(&cwd.canonicalize()?)?;
    let slot = env::var("ANTIQUOTA_SLOT").unwrap_or_else(|_| {
        PathBuf::from(env::var_os("HOME").unwrap())
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned()
    });
    let session = env::var("ANTIQUOTA_SESSION").unwrap_or_else(|_| slot.replace('@', "-"));
    let turn = sandbox::next_turn()?;
    let ctx = TurnContext {
        slot,
        cwd: cwd.to_string_lossy().into_owned(),
        conversation: commands::flag(&args, "--conversation")
            .map(str::to_owned)
            .unwrap_or_else(|| format!("conv-{session}-{turn}")),
        turn,
    };
    let scenario = script.selected(&ctx);
    if let Some(delay) = script.output_inherited_pipe_ms {
        let mut metadata = script.metadata_for(&ctx.slot).clone();
        metadata.inherited_pipe_ms = Some(delay);
        metadata.descendant_marker = script.output_descendant_marker.clone();
        descendant::spawn(&metadata)?;
    }
    let decision = if matches!(scenario, Scenario::Hook | Scenario::Approval) {
        hook::invoke(&script)?
    } else {
        None
    };
    let denied = decision.as_ref().is_some_and(|v| {
        v["decision"] == "deny"
            || v["permissionDecision"] == "deny"
            || v["hookSpecificOutput"]["permissionDecision"] == "deny"
    });
    if denied {
        script.reply = Some("Tool refused by the fixture hook.".into());
        script.write_notes = false;
    }
    let (mut frames, code) = protocol::frames(&script, &ctx);
    if denied {
        for frame in &mut frames {
            if frame.event["step_update"]["step_type"] == "tool"
                && frame.event["step_update"]["state"] == "DONE"
            {
                frame.event["step_update"]["state"] = json!("ERROR");
                frame.event["step_update"]["tool_info"]["error"] = json!({"type":"TOOL_ERROR","message":"permission check failed: user denied permission to run command"});
            }
        }
    }
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    if script.noise_bytes > 0 {
        // A runner that reads stderr only after stdout would block here on a full pipe.
        writeln!(output, "agy: warming up\n{{\"partial\": ")?;
        output.flush()?;
        std::io::stderr().write_all(&vec![b'e'; script.noise_bytes])?;
    }
    for frame in frames {
        if frame.delay_ms > 0 {
            thread::sleep(Duration::from_millis(frame.delay_ms));
        }
        if frame.event["step_update"]["step_type"] == "tool"
            && frame.event["step_update"]["state"] == "DONE"
            && script.write_notes
            && !matches!(
                scenario,
                Scenario::Auth | Scenario::Rate | Scenario::Hook | Scenario::Approval
            )
        {
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(cwd.join("notes.txt"))?;
            writeln!(file, "turn {turn}")?;
        }
        writeln!(output, "{}", frame.event)?;
        output.flush()?;
    }
    if scenario == Scenario::Crash {
        eprintln!("fake agy crashed");
    }
    Ok(code)
}
fn main() {
    let code = match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error}");
            2
        }
    };
    std::process::exit(code);
}
