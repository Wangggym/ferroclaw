use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ferroclaw_agent::{AgentConfig, AgentLoop};
use ferroclaw_agent::{ollama::OllamaProvider, openai::OpenAiProvider};
use ferroclaw_core::{ConversationHistory, Message, MessageContent, Role};
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
use std::io;

use crate::helpers::{
    build_memory_context, build_system_prompt, require_openai_key, tool_names_str,
    try_store_memory,
};

pub struct ChatState {
    pub messages: Vec<(String, String)>,
    pub input: String,
    pub status: String,
}

pub async fn run_chat(resume_session: Option<String>, no_memory: bool) -> Result<()> {
    let cfg = AgentConfig::load()?;
    let sm = SessionManager::open_default().await?;
    let max_steps = cfg.max_steps();
    let model = cfg.model_name().to_owned();

    let session_id = if let Some(ref sid) = resume_session {
        ferroclaw_core::SessionId::from_string(sid.clone())
    } else {
        sm.create_session().await?
    };

    let mut history = ConversationHistory::new();
    let existing = sm.load_history(&session_id).await?;

    let mut registry = ToolRegistry::new();
    registry.register(BashExecTool::new());
    let tool_names = tool_names_str(&registry);

    let mut display: Vec<(String, String)> = Vec::new();

    if existing.is_empty() {
        let system = build_system_prompt(&tool_names, "");
        history.push(Message::system(system));
    } else {
        for msg in existing {
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

    // Build provider once outside the event loop — avoids per-turn reqwest::Client recreation
    enum Provider {
        OpenAi(OpenAiProvider),
        Ollama(OllamaProvider),
    }

    let provider = match cfg.backend {
        ferroclaw_agent::LlmBackend::OpenAi => {
            let key = require_openai_key(&cfg)?;
            Provider::OpenAi(OpenAiProvider::new(key, model, cfg.openai_base_url.clone()))
        }
        ferroclaw_agent::LlmBackend::Ollama => {
            Provider::Ollama(OllamaProvider::new(model, cfg.ollama_base_url.clone()))
        }
    };

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut state = ChatState {
        messages: display,
        input: String::new(),
        status: format!("Session: {}  (Ctrl-C / Ctrl-D to quit)", session_id.as_str()),
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

                        // Inject relevant memories into system prompt once
                        if !no_memory && history.len() == 1 {
                            if let Ok(ctx) = build_memory_context(&cfg, &user_msg).await {
                                if !ctx.is_empty() {
                                    history.append_to_system_prompt(&ctx);
                                }
                            }
                        }

                        history.push(Message::user(user_msg.clone()));
                        sm.append_message(&session_id, history.as_slice().last().unwrap())
                            .await?;
                        state.messages.push(("user".to_owned(), user_msg.clone()));

                        let reply_result = match &provider {
                            Provider::OpenAi(p) => {
                                let agent = AgentLoop::new(p, &registry, max_steps);
                                agent.run(&mut history).await
                            }
                            Provider::Ollama(p) => {
                                let agent = AgentLoop::new(p, &registry, max_steps);
                                agent.run(&mut history).await
                            }
                        };

                        match reply_result {
                            Ok(reply) => {
                                sm.append_message(
                                    &session_id,
                                    history.as_slice().last().unwrap(),
                                )
                                .await?;
                                state.messages.push(("assistant".to_owned(), reply.clone()));

                                if !no_memory {
                                    let summary =
                                        format!("User: {user_msg}\nAssistant: {reply}");
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

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    println!("Session saved: {}", session_id.as_str());
    Ok(())
}

pub fn draw_chat(f: &mut ratatui::Frame, state: &ChatState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(f.area());

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

    let input = Paragraph::new(state.input.as_str())
        .block(Block::default().borders(Borders::ALL).title(" Message "))
        .wrap(Wrap { trim: false });
    f.render_widget(input, chunks[1]);

    let status =
        Paragraph::new(state.status.as_str()).style(Style::default().fg(Color::DarkGray));
    f.render_widget(status, chunks[2]);
}
