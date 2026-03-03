use anyhow::Result;
use clap::{Parser, Subcommand};

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
    /// Run an agent task (single-shot)
    Agent {
        /// Task message
        #[arg(short, long)]
        message: String,
    },
    /// Start an interactive chat session
    Chat,
    /// Onboard: configure API key and model
    Onboard,
}

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
        Some(Commands::Agent { message }) => {
            println!("agent: {message}");
            println!("[not yet implemented]");
        }
        Some(Commands::Chat) => {
            println!("chat: [not yet implemented]");
        }
        Some(Commands::Onboard) => {
            println!("onboard: [not yet implemented]");
        }
        None => {
            println!("ferroclaw — Personal AI Assistant (Rust)");
            println!("Run `ferroclaw --help` for usage.");
        }
    }

    Ok(())
}
