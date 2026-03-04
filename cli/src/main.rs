mod cmd;
mod helpers;

use anyhow::Result;
use clap::{Parser, Subcommand};

use cmd::{
    agent::run_agent,
    chat::run_chat,
    memory::MemoryCommands,
    memory::run_memory,
    onboard::run_onboard,
    sessions::SessionCommands,
    sessions::run_sessions,
};

// ── CLI definition ────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "ferroclaw",
    version,
    about = "Personal AI Assistant — local-first, single binary",
    long_about = None,
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a single agent task (streaming output)
    Agent {
        /// Task message
        #[arg(short, long)]
        message: String,
        /// Disable memory (skip context injection and memory storage)
        #[arg(long)]
        no_memory: bool,
    },
    /// Start an interactive multi-turn chat session (TUI)
    Chat {
        /// Resume an existing session by ID
        #[arg(long)]
        session: Option<String>,
        /// Disable memory integration
        #[arg(long)]
        no_memory: bool,
    },
    /// Configure API key and model interactively
    Onboard,
    /// Manage sessions
    Sessions {
        #[command(subcommand)]
        action: SessionCommands,
    },
    /// Manage long-term memory
    Memory {
        #[command(subcommand)]
        action: MemoryCommands,
    },
}

// ── entry point ───────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Agent { message, no_memory }) => run_agent(message, no_memory).await?,
        Some(Commands::Chat { session, no_memory }) => run_chat(session, no_memory).await?,
        Some(Commands::Onboard) => run_onboard().await?,
        Some(Commands::Sessions { action }) => run_sessions(action).await?,
        Some(Commands::Memory { action }) => run_memory(action).await?,
        None => {
            println!("ferroclaw — Personal AI Assistant (Rust)");
            println!("Run `ferroclaw --help` for usage.");
        }
    }

    Ok(())
}
