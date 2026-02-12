mod api;
mod app;
mod commands;
mod config;
mod cute;
mod errors;
mod output;
mod parse;
mod tui;

use clap::{Parser, Subcommand};

use crate::app::Runtime;
use crate::commands::auth::AuthCommand;
use crate::commands::billing::BillingCommand;
use crate::commands::chat::ChatArgs;
use crate::commands::config::ConfigCommand;
use crate::commands::tools::ToolsCommand;
use crate::commands::tui::TuiArgs;
use crate::commands::usage::UsageArgs;
use crate::errors::CliError;
use crate::output::{OutputMode, print_error};
use crate::commands::workspaces::WorkspaceCommand;

#[derive(Debug, Parser)]
#[command(
    name = "starbott",
    version,
    about = "Starbot CLI for chat, usage, billing, and account operations."
)]
struct Cli {
    #[arg(long, global = true)]
    profile: Option<String>,
    #[arg(long = "api-url", global = true)]
    api_url: Option<String>,
    #[arg(long, global = true)]
    json: bool,
    #[arg(long, global = true)]
    quiet: bool,
    #[arg(long, global = true, default_value_t = 30_000)]
    timeout: u64,
    #[arg(long, global = true, default_value_t = 2)]
    retries: u32,
    #[arg(long, global = true)]
    verbose: bool,
    #[arg(long, global = true)]
    debug: bool,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
    Workspaces {
        #[command(subcommand)]
        command: WorkspaceCommand,
    },
    Tools {
        #[command(subcommand)]
        command: ToolsCommand,
    },
    Whoami,
    Chat(ChatArgs),
    Tui(TuiArgs),
    Usage(UsageArgs),
    Billing {
        #[command(subcommand)]
        command: BillingCommand,
    },
    Health,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let output = OutputMode {
        json: cli.json,
        quiet: cli.quiet,
        verbose: cli.verbose,
        debug: cli.debug,
    };

    cute::print_banner(&output);

    let result = run(cli, output.clone()).await;
    if let Err(err) = result {
        print_error(&err, &output);
        std::process::exit(err.exit_code());
    }
}

async fn run(cli: Cli, output: OutputMode) -> Result<(), CliError> {
    let config = config::load_config()?;
    let config_path = config::config_path()?;

    let mut runtime = Runtime {
        output,
        config,
        config_path,
        profile_override: cli.profile,
        api_url_override: cli.api_url,
        timeout_ms: cli.timeout,
        retries: cli.retries,
    };

    match cli.command {
        Commands::Config { command } => commands::config::handle(&mut runtime, command).await,
        Commands::Auth { command } => commands::auth::handle(&mut runtime, command).await,
        Commands::Workspaces { command } => commands::workspaces::handle(&runtime, command).await,
        Commands::Tools { command } => commands::tools::handle(&runtime, command).await,
        Commands::Whoami => commands::whoami::handle(&runtime).await,
        Commands::Chat(args) => commands::chat::handle(&runtime, args).await,
        Commands::Tui(args) => commands::tui::handle(&runtime, args).await,
        Commands::Usage(args) => commands::usage::handle(&runtime, args).await,
        Commands::Billing { command } => commands::billing::handle(&runtime, command).await,
        Commands::Health => commands::health::handle(&runtime).await,
    }
}
