use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ferroclaw_agent::ollama::OllamaProvider;
use ferroclaw_agent::openai::OpenAiProvider;
use ferroclaw_agent::{AgentConfig, AgentLoop, LlmBackend};
use ferroclaw_core::ConversationHistory;
use ferroclaw_memory::{
    retrieve_context, store_conversation_memory, EmbeddingProvider, MemoryManager, OpenAiEmbedding,
};
use ferroclaw_session::SessionManager;
use ferroclaw_tools::{BashExecTool, ToolRegistry};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Terminal,
};
use std::io::{self, Write as IoWrite};

// ── CLI definition ───────────────────────────────────────────────────────────

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

#[derive(Subcommand)]
enum SessionCommands {
    /// List all sessions
    List,
    /// Clear all sessions
    Clear,
}

#[derive(Subcommand)]
enum MemoryCommands {
    /// List all memory entries
    List,
    /// Search memory for a query
    Search {
        /// The query to search for
        query: String,
        /// Number of results
        #[arg(short, long, default_value = "5")]
        top_k: usize,
    },
    /// Forget (delete) a memory entry by ID
    Forget { id: String },
    /// Clear all memory entries
    Clear,
}

// ── entry point ──────────────────────────────────────────────────────────────

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

// ── agent (single-shot) ──────────────────────────────────────────────────────

async fn run_agent(message: String, no_memory: bool) -> Result<()> {
    let cfg = AgentConfig::load()?;
    let mut registry = ToolRegistry::new();
    registry.register(BashExecTool::new());

    // Optionally retrieve memory context
    let memory_ctx = if !no_memory {
        build_memory_context(&cfg, &message)
            .await
            .unwrap_or_default()
    } else {
        String::new()
    };

    let mut history = ConversationHistory::new();
    let tool_names = tool_names_str(&registry);
    let system = build_system_prompt(&tool_names, &memory_ctx);
    history.push(ferroclaw_core::Message::system(system));
    history.push(ferroclaw_core::Message::user(message.clone()));

    let max_steps = cfg.max_steps();
    let model = cfg.model_name().to_owned();

    let reply = match cfg.backend {
        LlmBackend::OpenAi => {
            let key = require_openai_key(&cfg)?;
            let provider = OpenAiProvider::new(key, model, cfg.openai_base_url.clone());
            run_with_provider(&provider, &registry, &mut history, max_steps).await?
        }
        LlmBackend::Ollama => {
            let provider = OllamaProvider::new(model, cfg.ollama_base_url.clone());
            run_with_provider(&provider, &registry, &mut history, max_steps).await?
        }
    };

    println!("{reply}");

    // Store memory summary if enabled
    if !no_memory {
        let summary = format!("User: {message}\nAssistant: {reply}");
        try_store_memory(&cfg, &summary).await;
    }

    Ok(())
}

// ── interactive chat TUI ─────────────────────────────────────────────────────

struct ChatState {
    messages: Vec<(String, String)>, // (role, text)
    input: String,
    status: String,
}

async fn run_chat(resume_session: Option<String>, no_memory: bool) -> Result<()> {
    let cfg = AgentConfig::load()?;
    let sm = SessionManager::open_default().await?;

    // Create or resume session
    let session_id = if let Some(ref sid) = resume_session {
        ferroclaw_core::SessionId::from_string(sid.clone())
    } else {
        sm.create_session().await?
    };

    // Load existing history
    let mut history = ConversationHistory::new();
    let existing = sm.load_history(&session_id).await?;

    let mut registry = ToolRegistry::new();
    registry.register(BashExecTool::new());
    let tool_names = tool_names_str(&registry);

    // Build display messages from loaded history
    let mut display: Vec<(String, String)> = Vec::new();

    if existing.is_empty() {
        let system = build_system_prompt(&tool_names, "");
        history.push(ferroclaw_core::Message::system(system));
    } else {
        for msg in existing {
            use ferroclaw_core::{MessageContent, Role};
            let role_str = match msg.role {
                Role::System => "system",
                Role::User => "user",
                Role::Assistant => "assistant",
                Role::Tool => "tool",
            };
            let text = match &msg.content {
                MessageContent::Text(t) => t.clone(),
                _ => serde_json::to_string(&msg.content).unwrap_or_default(),
            };
            if matches!(msg.role, Role::User | Role::Assistant) {
                display.push((role_str.to_owned(), text));
            }
            history.push(msg);
        }
    }

    // TUI setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut state = ChatState {
        messages: display,
        input: String::new(),
        status: format!(
            "Session: {}  (Ctrl-C / Ctrl-D to quit)",
            session_id.as_str()
        ),
    };

    loop {
        terminal.draw(|f| draw_chat(f, &state))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match (key.code, key.modifiers) {
                    (KeyCode::Char('c'), KeyModifiers::CONTROL)
                    | (KeyCode::Char('d'), KeyModifiers::CONTROL) => break,

                    (KeyCode::Enter, _) => {
                        let user_msg = state.input.trim().to_owned();
                        if user_msg.is_empty() {
                            continue;
                        }
                        state.input.clear();
                        state.status = "Thinking…".to_owned();
                        terminal.draw(|f| draw_chat(f, &state))?;

                        // Retrieve memory context for this turn
                        let memory_ctx = if !no_memory {
                            build_memory_context(&cfg, &user_msg)
                                .await
                                .unwrap_or_default()
                        } else {
                            String::new()
                        };
                        // If we have relevant memories, update system prompt temporarily
                        if !memory_ctx.is_empty() && history.messages.len() == 1 {
                            if let Some(sys) = history.messages.first_mut() {
                                use ferroclaw_core::MessageContent;
                                if let MessageContent::Text(ref mut t) = sys.content {
                                    t.push('\n');
                                    t.push_str(&memory_ctx);
                                }
                            }
                        }

                        history.push(ferroclaw_core::Message::user(user_msg.clone()));
                        sm.append_message(&session_id, history.messages.last().unwrap())
                            .await?;
                        state.messages.push(("user".to_owned(), user_msg.clone()));

                        // Run agent
                        let model = cfg.model_name().to_owned();
                        let reply_result = match cfg.backend {
                            LlmBackend::OpenAi => match require_openai_key(&cfg) {
                                Ok(key) => {
                                    let provider = OpenAiProvider::new(
                                        key,
                                        model,
                                        cfg.openai_base_url.clone(),
                                    );
                                    run_with_provider(
                                        &provider,
                                        &registry,
                                        &mut history,
                                        cfg.max_steps(),
                                    )
                                    .await
                                }
                                Err(e) => Err(e),
                            },
                            LlmBackend::Ollama => {
                                let provider =
                                    OllamaProvider::new(model, cfg.ollama_base_url.clone());
                                run_with_provider(
                                    &provider,
                                    &registry,
                                    &mut history,
                                    cfg.max_steps(),
                                )
                                .await
                            }
                        };

                        match reply_result {
                            Ok(reply) => {
                                sm.append_message(&session_id, history.messages.last().unwrap())
                                    .await?;
                                state.messages.push(("assistant".to_owned(), reply.clone()));

                                // Store memory
                                if !no_memory {
                                    let summary = format!("User: {user_msg}\nAssistant: {reply}");
                                    try_store_memory(&cfg, &summary).await;
                                }
                            }
                            Err(e) => {
                                state.messages.push(("error".to_owned(), e.to_string()));
                            }
                        }
                        state.status = format!(
                            "Session: {}  (Ctrl-C / Ctrl-D to quit)",
                            session_id.as_str()
                        );
                    }

                    (KeyCode::Backspace, _) => {
                        state.input.pop();
                    }
                    (KeyCode::Char(c), _) => {
                        state.input.push(c);
                    }
                    _ => {}
                }
            }
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    println!("Session saved: {}", session_id.as_str());
    Ok(())
}

fn draw_chat(f: &mut ratatui::Frame, state: &ChatState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(f.area());

    // Chat history
    let items: Vec<ListItem> = state
        .messages
        .iter()
        .map(|(role, text)| {
            let color = match role.as_str() {
                "user" => Color::Cyan,
                "assistant" => Color::Green,
                _ => Color::Red,
            };
            let prefix = match role.as_str() {
                "user" => "You",
                "assistant" => "ferroclaw",
                _ => "!",
            };
            // Wrap long lines simply
            let label = format!("[{prefix}] {text}");
            ListItem::new(Line::from(Span::styled(label, Style::default().fg(color))))
        })
        .collect();

    let history = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" ferroclaw chat "),
        )
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));
    f.render_widget(history, chunks[0]);

    // Input box
    let input = Paragraph::new(state.input.as_str())
        .block(Block::default().borders(Borders::ALL).title(" Message "))
        .wrap(Wrap { trim: false });
    f.render_widget(input, chunks[1]);

    // Status bar
    let status = Paragraph::new(state.status.as_str()).style(Style::default().fg(Color::DarkGray));
    f.render_widget(status, chunks[2]);
}

// ── onboard ──────────────────────────────────────────────────────────────────

async fn run_onboard() -> Result<()> {
    println!("ferroclaw onboard — interactive setup");
    println!();

    let config_path = AgentConfig::default_config_path();
    println!("Config file: {}", config_path.display());
    println!();

    // Prompt for backend
    print!("Backend [openai/ollama] (default: openai): ");
    io::stdout().flush()?;
    let mut backend_str = String::new();
    io::stdin().read_line(&mut backend_str)?;
    let backend = backend_str.trim();
    let use_ollama = backend.eq_ignore_ascii_case("ollama");

    // Prompt for API key if OpenAI
    let (api_key, base_url) = if use_ollama {
        print!("Ollama base URL (default: http://localhost:11434): ");
        io::stdout().flush()?;
        let mut url = String::new();
        io::stdin().read_line(&mut url)?;
        let url = url.trim().to_owned();
        let base = if url.is_empty() { None } else { Some(url) };
        (None::<String>, base)
    } else {
        print!("OpenAI API key (sk-...): ");
        io::stdout().flush()?;
        let mut key = String::new();
        io::stdin().read_line(&mut key)?;
        let key = key.trim().to_owned();
        print!("OpenAI base URL (leave blank for default): ");
        io::stdout().flush()?;
        let mut url = String::new();
        io::stdin().read_line(&mut url)?;
        let url = url.trim().to_owned();
        let base = if url.is_empty() { None } else { Some(url) };
        (if key.is_empty() { None } else { Some(key) }, base)
    };

    // Prompt for model
    let default_model = if use_ollama { "llama3" } else { "gpt-4o" };
    print!("Model (default: {default_model}): ");
    io::stdout().flush()?;
    let mut model_str = String::new();
    io::stdin().read_line(&mut model_str)?;
    let model = model_str.trim().to_owned();
    let model = if model.is_empty() { None } else { Some(model) };

    // Write config
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut lines = Vec::new();
    lines.push(format!(
        "backend = \"{}\"",
        if use_ollama { "ollama" } else { "openai" }
    ));
    if let Some(m) = model {
        lines.push(format!("model = \"{m}\""));
    }
    if let Some(key) = api_key {
        lines.push(format!("openai_api_key = \"{key}\""));
    }
    if let Some(url) = base_url {
        if use_ollama {
            lines.push(format!("ollama_base_url = \"{url}\""));
        } else {
            lines.push(format!("openai_base_url = \"{url}\""));
        }
    }

    std::fs::write(&config_path, lines.join("\n") + "\n")?;
    println!();
    println!("Config written to: {}", config_path.display());
    println!("Run: ferroclaw agent -m \"hello\"");
    Ok(())
}

// ── sessions ─────────────────────────────────────────────────────────────────

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

// ── memory ───────────────────────────────────────────────────────────────────

async fn run_memory(action: MemoryCommands) -> Result<()> {
    let mm = MemoryManager::open_default().await?;
    match action {
        MemoryCommands::List => {
            let entries = mm.list().await?;
            if entries.is_empty() {
                println!("No memory entries.");
            } else {
                println!("{:<36}  {:<20}  Content", "ID", "Created");
                println!("{}", "-".repeat(80));
                for (id, content, created) in entries {
                    let preview = if content.chars().count() > 40 {
                        let cut: String = content.chars().take(40).collect();
                        format!("{cut}…")
                    } else {
                        content.clone()
                    };
                    println!("{id:<36}  {created:<20}  {preview}");
                }
            }
        }
        MemoryCommands::Search { query, top_k } => {
            let cfg = AgentConfig::load()?;
            let embedder = make_embedder(&cfg)?;
            let results = {
                let emb: Vec<f32> = embedder.embed(&query).await?;
                mm.search(&emb, top_k).await?
            };
            if results.is_empty() {
                println!("No relevant memories found.");
            } else {
                for (i, entry) in results.iter().enumerate() {
                    println!("{}. [score: {:.3}] [id: {}]", i + 1, entry.score, entry.id);
                    println!("   {}", entry.content);
                    println!();
                }
            }
        }
        MemoryCommands::Forget { id } => {
            let deleted = mm.forget(&id).await?;
            if deleted {
                println!("Forgotten: {id}");
            } else {
                println!("Not found: {id}");
            }
        }
        MemoryCommands::Clear => {
            let n = mm.clear_all().await?;
            println!("Cleared {n} memory entry/entries.");
        }
    }
    Ok(())
}

// ── helpers ──────────────────────────────────────────────────────────────────

async fn run_with_provider<P: ferroclaw_agent::LlmProvider>(
    provider: &P,
    registry: &ToolRegistry,
    history: &mut ConversationHistory,
    max_steps: usize,
) -> Result<String> {
    let agent = AgentLoop::new(provider, registry, max_steps);
    let response = agent.run(history).await?;
    Ok(response)
}

fn tool_names_str(registry: &ToolRegistry) -> String {
    registry
        .schemas()
        .iter()
        .map(|s| s["function"]["name"].as_str().unwrap_or("").to_owned())
        .collect::<Vec<_>>()
        .join(", ")
}

fn build_system_prompt(tool_names: &str, memory_ctx: &str) -> String {
    let mut prompt = format!(
        "You are ferroclaw, a local-first AI assistant. \
         You have access to these tools: {tool_names}. \
         Use them to help the user."
    );
    if !memory_ctx.is_empty() {
        prompt.push('\n');
        prompt.push_str(memory_ctx);
    }
    prompt
}

fn require_openai_key(cfg: &AgentConfig) -> Result<String> {
    cfg.openai_api_key.clone().context(
        "OpenAI API key not set. Add FERROCLAW_OPENAI_API_KEY to .env or run `ferroclaw onboard`.",
    )
}

fn make_embedder(cfg: &AgentConfig) -> Result<OpenAiEmbedding> {
    let key = require_openai_key(cfg)?;
    Ok(OpenAiEmbedding::new(key, cfg.openai_base_url.clone()))
}

async fn build_memory_context(cfg: &AgentConfig, query: &str) -> Result<String> {
    if cfg.openai_api_key.is_none() {
        return Ok(String::new());
    }
    let mm = MemoryManager::open_default().await?;
    let embedder = make_embedder(cfg)?;
    let ctx = retrieve_context(&mm, &embedder, query, 5).await?;
    Ok(ctx)
}

async fn try_store_memory(cfg: &AgentConfig, summary: &str) {
    if cfg.openai_api_key.is_none() {
        return;
    }
    if let Ok(mm) = MemoryManager::open_default().await {
        if let Ok(embedder) = make_embedder(cfg) {
            let _ = store_conversation_memory(&mm, &embedder, summary).await;
        }
    }
}
