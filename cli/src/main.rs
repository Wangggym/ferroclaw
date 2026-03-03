use anyhow::Result;
use clap::{Parser, Subcommand};
use ferroclaw_agent::{AgentConfig, AgentLoop, LlmBackend};
use ferroclaw_agent::openai::OpenAiProvider;
use ferroclaw_agent::ollama::OllamaProvider;
use ferroclaw_core::ConversationHistory;
use ferroclaw_session::SessionManager;
use ferroclaw_tools::{BashExecTool, ToolRegistry};

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
    },
    /// Start an interactive chat session
    Chat,
    /// Onboard: configure API key and model
    Onboard,
    /// Manage sessions
    Sessions {
        #[command(subcommand)]
        action: SessionCommands,
    },
}

#[derive(Subcommand)]
enum SessionCommands {
    /// List all sessions
    List,
    /// Clear all sessions
    Clear,
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
        Some(Commands::Agent { message }) => run_agent(message).await?,
        Some(Commands::Chat) => {
            println!("chat: [not yet implemented — coming in P1-H]");
        }
        Some(Commands::Onboard) => run_onboard().await?,
        Some(Commands::Sessions { action }) => run_sessions(action).await?,
        None => {
            println!("ferroclaw — Personal AI Assistant (Rust)");
            println!("Run `ferroclaw --help` for usage.");
        }
    }

    Ok(())
}

async fn run_agent(message: String) -> Result<()> {
    let cfg = AgentConfig::load()?;

    // Build registry with bash_exec
    let mut registry = ToolRegistry::new();
    registry.register(BashExecTool::new());

    // Build history with system prompt + user message
    let mut history = ConversationHistory::new();
    let tool_names = registry
        .schemas()
        .iter()
        .map(|s| s["function"]["name"].as_str().unwrap_or("").to_owned())
        .collect::<Vec<_>>()
        .join(", ");

    history.push(ferroclaw_core::Message::system(format!(
        "You are ferroclaw, a local-first AI assistant. \
         You have access to these tools: {tool_names}. \
         Use them to help the user."
    )));
    history.push(ferroclaw_core::Message::user(message));

    let max_steps = cfg.max_steps();
    let model = cfg.model_name().to_owned();

    match cfg.backend {
        LlmBackend::OpenAi => {
            let key = cfg.openai_api_key.clone().ok_or_else(|| {
                anyhow::anyhow!(
                    "OpenAI API key not set. Add FERROCLAW_OPENAI_API_KEY to .env or config."
                )
            })?;
            let provider = OpenAiProvider::new(key, model, cfg.openai_base_url.clone());
            run_with_provider(&provider, &registry, &mut history, max_steps).await?;
        }
        LlmBackend::Ollama => {
            let provider = OllamaProvider::new(model, cfg.ollama_base_url.clone());
            run_with_provider(&provider, &registry, &mut history, max_steps).await?;
        }
    }

    Ok(())
}

async fn run_with_provider<P: ferroclaw_agent::LlmProvider>(
    provider: &P,
    registry: &ToolRegistry,
    history: &mut ConversationHistory,
    max_steps: usize,
) -> Result<()> {
    let agent = AgentLoop::new(provider, registry, max_steps);
    let response = agent.run(history).await?;
    println!("{response}");
    Ok(())
}

async fn run_onboard() -> Result<()> {
    let config_path = AgentConfig::default_config_path();
    println!("ferroclaw onboard");
    println!();
    println!("Config file: {}", config_path.display());
    println!();
    println!("Set your API key via environment variable:");
    println!("  export FERROCLAW_OPENAI_API_KEY=sk-...");
    println!();
    println!("Or create the config file:");
    println!("  backend = \"openai\"");
    println!("  model = \"gpt-4o\"");
    println!("  openai_api_key = \"sk-...\"");
    println!();
    println!("Then run: ferroclaw agent -m \"hello\"");
    Ok(())
}

async fn run_sessions(action: SessionCommands) -> Result<()> {
    let sm = SessionManager::open_default().await?;
    match action {
        SessionCommands::List => {
            let sessions = sm.list_sessions().await?;
            if sessions.is_empty() {
                println!("No sessions.");
            } else {
                println!("{:<36}  {:<20}  {}", "ID", "Updated", "Title");
                println!("{}", "-".repeat(70));
                for (id, title, updated) in sessions {
                    println!(
                        "{:<36}  {:<20}  {}",
                        id,
                        updated,
                        title.as_deref().unwrap_or("-")
                    );
                }
            }
        }
        SessionCommands::Clear => {
            let n = sm.clear_all().await?;
            println!("Cleared {n} session(s).");
        }
    }
    Ok(())
}
