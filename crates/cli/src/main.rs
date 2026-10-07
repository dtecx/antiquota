//! Owns command-line dispatch; the hook runs before logging and Tokio startup.
mod accounts;
mod goal;
mod hook;
mod raw;
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
fn main() -> anyhow::Result<()> {
    if std::env::args().nth(1).as_deref() == Some("hook") {
        return hook::run(&std::env::args().nth(2).unwrap_or_else(|| "pre-tool".into()));
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
    let mut code = 0;
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
