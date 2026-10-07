//! Owns command-line dispatch; the hook runs before logging and Tokio startup.
mod run;
use clap::{Parser, Subcommand};
#[derive(Parser)]
#[command(
    name = "antiquota",
    version,
    about = "A local workspace for agy sessions"
)]
struct Cli {
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
    Run {
        #[arg(long)]
        cwd: Option<String>,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        slot: Option<String>,
        #[arg(required=true,num_args=1..)]
        text: Vec<String>,
    },
    Hook {
        #[arg(default_value = "pre-tool")]
        kind: String,
    },
}
fn main() -> anyhow::Result<()> {
    if std::env::args().nth(1).as_deref() == Some("hook") {
        return hook();
    }
    let cli = Cli::parse();
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
    match cli.command.unwrap_or(Commands::Gui {
        port: 8765,
        no_open: false,
    }) {
        Commands::Serve { port, .. } => runtime.block_on(antiquota_server::serve(port, false))?,
        Commands::Gui { port, no_open } => {
            runtime.block_on(antiquota_server::serve(port, !no_open))?
        }
        Commands::Run {
            cwd,
            model,
            slot,
            text,
        } => {
            let info = runtime.block_on(antiquota_server::discover())?;
            run::run(info, cwd, model, slot, text.join(" "))?;
        }
        Commands::Hook { .. } => hook()?,
    }
    Ok(())
}
fn hook() -> anyhow::Result<()> {
    use std::io::{Read, Write};
    let mut bytes = vec![];
    std::io::stdin().take(1_000_001).read_to_end(&mut bytes)?;
    let reason = if bytes.len() > 1_000_000 {
        "Hook input exceeds the limit."
    } else {
        "Approval support is not available in this build; the tool is denied."
    };
    serde_json::to_writer(
        std::io::stdout(),
        &serde_json::json!({"decision":"deny","reason":reason}),
    )?;
    std::io::stdout().write_all(b"\n")?;
    Ok(())
}
