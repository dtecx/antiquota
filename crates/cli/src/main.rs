//! Owns command-line dispatch; the hook runs before logging and Tokio startup.
mod accounts;
mod commands;
mod engine;
mod follow;
mod goal;
mod hook;
mod interactive;
mod raw;
mod terminal;
use clap::{CommandFactory, Parser, Subcommand};
#[derive(Parser)]
#[command(
    name = "antiquota",
    version,
    about = "A local workspace for agy sessions",
    args_conflicts_with_subcommands = true
)]
struct Cli {
    #[command(flatten)]
    interactive: interactive::InteractiveArgs,
    #[command(subcommand)]
    command: Option<Commands>,
}
#[derive(Subcommand)]
enum Commands {
    Serve {
        #[arg(long, default_value_t = 8765)]
        port: u16,
        #[arg(long)]
        no_open: bool,
    },
    Gui {
        #[arg(long, default_value_t = 8765)]
        port: u16,
        #[arg(long)]
        no_open: bool,
    },
    Accounts,
    Login,
    Probe,
    Doctor,
    Stack {
        #[arg(value_parser=["reseed"])]
        action: Option<String>,
    },
    Unwall {
        name: String,
    },
    Run {
        #[arg(trailing_var_arg=true,allow_hyphen_values=true,num_args=0..)]
        args: Vec<String>,
    },
    Hook {
        #[arg(default_value = "pre-tool")]
        kind: String,
    },
    /// Work on an objective until it is verified done.
    Goal(goal::GoalArgs),
}
/// The PATH of the user's login shell. An app started from the Dock or a desktop menu gets a
/// bare PATH, so commands would resolve differently than in the user's terminal. The shell has
/// five seconds; its output may carry other text, so the PATH follows a marker.
#[cfg(unix)]
fn login_path() -> Option<String> {
    use std::{
        io::Read,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let shell = std::env::var("SHELL")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "/bin/zsh".into());
    let mut child = Command::new(shell)
        .args(["-ilc", r#"printf "\n__PATH__%s" "$PATH""#])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stdout.read_to_string(&mut text);
        let _ = tx.send(text);
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while child.try_wait().ok()?.is_none() {
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let text = rx.recv_timeout(Duration::from_secs(1)).ok()?;
    let (_, path) = text.rsplit_once("__PATH__")?;
    Some(path.trim().to_owned()).filter(|p| !p.is_empty())
}
fn main() -> anyhow::Result<()> {
    if std::env::args().nth(1).as_deref() == Some("hook") {
        return hook::run(&std::env::args().nth(2).unwrap_or_else(|| "pre-tool".into()));
    }
    let cli = Cli::try_parse().unwrap_or_else(|error| {
        // Every usage error shows the usage line, as the legacy CLI did; clap leaves it out of
        // some, such as an invalid value. The error then exits 2.
        if error.use_stderr() && !error.to_string().contains("Usage:") {
            eprintln!("{}", Cli::command().render_usage());
        }
        error.exit()
    });
    #[cfg(unix)]
    if std::env::var_os("ANTIQUOTA_FIX_PATH").is_some()
        && matches!(
            cli.command,
            Some(Commands::Serve { .. } | Commands::Gui { .. })
        )
        && let Some(path) = login_path()
    {
        use std::os::unix::process::CommandExt;
        // Start again once with the login PATH; clearing the variable makes it happen once.
        let error = std::process::Command::new(std::env::current_exe()?)
            .args(std::env::args_os().skip(1))
            .env("PATH", path)
            .env_remove("ANTIQUOTA_FIX_PATH")
            .exec();
        eprintln!("antiquota: the login shell's PATH could not be used: {error}");
    }
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "antiquota=info".into()),
        )
        .with_writer(std::io::stderr)
        .init();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let mut code = 0;
    if cli.interactive.interactive {
        runtime.block_on(interactive::run(cli.interactive))?;
        return Ok(());
    }
    match cli.command.unwrap_or(Commands::Accounts) {
        Commands::Serve { port, .. } => runtime.block_on(antiquota_server::serve(port, false))?,
        Commands::Gui { port, no_open } => {
            runtime.block_on(antiquota_server::serve(port, !no_open))?
        }
        Commands::Accounts => accounts::print(accounts::context()?.as_ref())?,
        Commands::Login => code = runtime.block_on(accounts::login(accounts::context()?))?,
        Commands::Probe => runtime.block_on(accounts::probe(accounts::context()?))?,
        Commands::Doctor => code = runtime.block_on(accounts::doctor(accounts::context()?))?,
        Commands::Stack { action } => accounts::stack(
            accounts::context()?.as_ref(),
            action.as_deref() == Some("reseed"),
        )?,
        Commands::Unwall { name } => accounts::unwall(accounts::context()?.as_ref(), &name)?,
        Commands::Run { args } => code = runtime.block_on(raw::run(accounts::context()?, args))?,
        Commands::Hook { kind } => hook::run(&kind)?,
        Commands::Goal(args) => code = runtime.block_on(goal::run(args))?,
    }
    if code != 0 {
        std::process::exit(code);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Option<Commands> {
        Cli::try_parse_from(std::iter::once("antiquota").chain(args.iter().copied()))
            .unwrap()
            .command
    }

    #[test]
    fn every_command_parses_with_its_legacy_defaults() {
        assert!(parse(&[]).is_none(), "no command lists the accounts");
        assert!(matches!(
            parse(&["serve"]),
            Some(Commands::Serve {
                port: 8765,
                no_open: false
            })
        ));
        assert!(matches!(
            parse(&["gui"]),
            Some(Commands::Gui {
                port: 8765,
                no_open: false
            })
        ));
        assert!(matches!(
            parse(&["gui", "--port", "9000", "--no-open"]),
            Some(Commands::Gui {
                port: 9000,
                no_open: true
            })
        ));
        assert!(matches!(parse(&["accounts"]), Some(Commands::Accounts)));
        assert!(matches!(parse(&["login"]), Some(Commands::Login)));
        assert!(matches!(parse(&["probe"]), Some(Commands::Probe)));
        assert!(matches!(parse(&["doctor"]), Some(Commands::Doctor)));
        assert!(matches!(
            parse(&["stack"]),
            Some(Commands::Stack { action: None })
        ));
        assert!(matches!(
            parse(&["stack", "reseed"]),
            Some(Commands::Stack { action: Some(a) }) if a == "reseed"
        ));
        assert!(matches!(
            parse(&["unwall", "one@example.com"]),
            Some(Commands::Unwall { name }) if name == "one@example.com"
        ));
        assert!(matches!(
            parse(&["hook"]),
            Some(Commands::Hook { kind }) if kind == "pre-tool"
        ));
        assert!(matches!(
            parse(&["hook", "pre-invocation"]),
            Some(Commands::Hook { kind }) if kind == "pre-invocation"
        ));
        assert!(matches!(
            parse(&["goal", "Ship it"]),
            Some(Commands::Goal(_))
        ));
    }

    #[test]
    fn interactive_options_are_additive_and_do_not_steal_raw_run_arguments() {
        let line = Cli::try_parse_from(["antiquota", "-i", "--resume", "session-123"]).unwrap();
        assert!(line.interactive.interactive);
        assert!(line.command.is_none());
        for args in [
            vec!["--resume", "x"],
            vec!["-i", "accounts"],
            vec!["-i", "--resume", "x", "--model", "m"],
        ] {
            assert!(Cli::try_parse_from(std::iter::once("antiquota").chain(args)).is_err());
        }
        let line =
            Cli::try_parse_from(["antiquota", "run", "--resume", "x", "--model", "m"]).unwrap();
        assert!(
            matches!(line.command, Some(Commands::Run { args }) if args == ["--resume", "x", "--model", "m"])
        );
    }

    #[test]
    fn run_passes_every_later_word_to_agy_and_unknown_commands_fail() {
        let Some(Commands::Run { args }) = parse(&["run", "-p", "hi", "--model", "m", "x"]) else {
            panic!("run did not parse");
        };
        assert_eq!(args, ["-p", "hi", "--model", "m", "x"]);
        for line in [
            &["stack", "wipe"][..],
            &["chat"],
            &["unwall"],
            &["serve", "--port", "x"],
        ] {
            let words = std::iter::once("antiquota").chain(line.iter().copied());
            assert!(Cli::try_parse_from(words).is_err(), "{line:?}");
        }
    }
}
