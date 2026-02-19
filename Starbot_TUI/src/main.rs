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
use crate::commands::animate::AnimateArgs;
use crate::commands::agent::CLIAgentCommands;
use crate::commands::auth::AuthCommand;
use crate::commands::billing::BillingCommand;
use crate::commands::chat::ChatArgs;
use crate::commands::config::ConfigCommand;
use crate::commands::tasks::TaskCommands;
use crate::commands::tools::ToolsCommand;
use crate::commands::tui::TuiArgs;
use crate::commands::usage::UsageArgs;
use crate::errors::CliError;
use crate::output::{OutputMode, print_error};
use crate::commands::workspaces::WorkspaceCommand;
use serde_json::json;

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
    Animate(AnimateArgs),
    Health,
    Tasks {
        #[command(subcommand)]
        command: TaskCommands,
    },
    Agent {
        #[command(subcommand)]
        command: AgentCommand,
    },
}

#[derive(Debug, Subcommand)]
enum AgentCommand {
    /// Initialize a new agent session
    Init(InitArgs),
    /// Process a task with the agent
    Process(ProcessArgs),
    /// Run the agent with a prompt (creates chat, streams generation)
    Run(RunArgs),
}

#[derive(Debug, clap::Args)]
struct InitArgs {
    /// Model to use
    #[arg(long, default_value = "gpt-4")]
    model: String,
    /// Enable task management
    #[arg(long)]
    enable_tasks: bool,
}

#[derive(Debug, clap::Args)]
struct ProcessArgs {
    /// Task ID to process
    #[arg(required = true)]
    task_id: String,
    /// Model to use
    #[arg(long, default_value = "gpt-4")]
    model: String,
}

#[derive(Debug, clap::Args)]
struct RunArgs {
    /// Prompt to send to the agent
    pub prompt: String,
    /// Project ID to scope the chat
    #[arg(long)]
    pub project_id: Option<String>,
    /// Existing chat ID to continue
    #[arg(long)]
    pub chat_id: Option<String>,
    /// Model preference (e.g. "azure:gpt-4o")
    #[arg(short = 'm', long)]
    pub model: Option<String>,
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
        Commands::Animate(args) => commands::animate::handle(&runtime, args).await,
        Commands::Health => commands::health::handle(&runtime).await,
        Commands::Tasks { command } => commands::tasks::handle_tasks(&runtime, command).await,
        Commands::Agent { command } => handle_agent_command(&mut runtime, command).await,
    }
}

async fn handle_agent_command(runtime: &mut Runtime, command: AgentCommand) -> Result<(), CliError> {
    let api = runtime.api_client()?;

    match command {
        AgentCommand::Init(args) => {
            let config = crate::commands::agent::AgentConfig {
                model: args.model,
                max_tokens: 4096,
                temperature: 0.7,
                enable_tasks: args.enable_tasks,
                ..Default::default()
            };

            match CLIAgentCommands::create(config, api).await {
                Ok(_agent) => {
                    if runtime.output.json {
                        runtime.output.print_json(&json!({ "success": true, "message": "Agent initialized" }))?;
                    } else {
                        runtime.output.print_human("✓ Agent initialized successfully");
                    }
                }
                Err(e) => return Err(e),
            }
        }
        AgentCommand::Process(args) => {
            let config = crate::commands::agent::AgentConfig {
                model: args.model,
                ..Default::default()
            };

            match CLIAgentCommands::create(config, api).await {
                Ok(mut agent) => {
                    match CLIAgentCommands::process_task(&mut agent, &args.task_id).await {
                        Ok(_) => {
                            if runtime.output.json {
                                runtime.output.print_json(&json!({ "success": true, "message": "Task processed" }))?;
                            } else {
                                runtime.output.print_human("✓ Task processed successfully");
                            }
                        }
                        Err(e) => return Err(e),
                    }
                }
                Err(e) => return Err(e),
            }
        }
        AgentCommand::Run(args) => {
            crate::commands::agent::handle_run(
                &api,
                &runtime,
                args.prompt,
                args.project_id,
                args.chat_id,
                args.model,
            ).await?;
        }
    }

    Ok(())
}
