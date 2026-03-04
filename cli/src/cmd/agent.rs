use anyhow::Result;
use ferroclaw_agent::provider::LlmChunk;
use ferroclaw_agent::{ollama::OllamaProvider, openai::OpenAiProvider, AgentConfig, AgentLoop, LlmProvider};
use ferroclaw_core::{ConversationHistory, Message};
use ferroclaw_tools::{BashExecTool, ToolRegistry};
use futures_util::{pin_mut, StreamExt};
use std::io::Write as _;

use crate::helpers::{
    build_memory_context, build_system_prompt, tool_names_str, try_store_memory, ProviderKind,
};

pub async fn run_agent(message: String, no_memory: bool) -> Result<()> {
    let cfg = AgentConfig::load()?;
    let mut registry = ToolRegistry::new();
    registry.register(BashExecTool::new());

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
    history.push(Message::system(system));
    history.push(Message::user(message.clone()));

    let max_steps = cfg.max_steps();
    let tools = registry.schemas();
    let provider = ProviderKind::from_config(&cfg)?;

    // Attempt streaming for the first turn to give typewriter output.
    // Falls back to the full AgentLoop when tool calls are involved.
    let mut streamed_text = String::new();
    let mut has_tool_calls = false;

    async fn stream_turn<P: LlmProvider>(
        p: &P,
        history: &ConversationHistory,
        tools: &[serde_json::Value],
        streamed_text: &mut String,
        has_tool_calls: &mut bool,
    ) -> Result<()> {
        let stream = p.complete_stream(history.as_slice(), tools);
        pin_mut!(stream);
        while let Some(chunk) = stream.next().await {
            match chunk.map_err(anyhow::Error::from)? {
                LlmChunk::Text(t) => {
                    print!("{t}");
                    std::io::stdout().flush()?;
                    streamed_text.push_str(&t);
                }
                LlmChunk::ToolCalls(_) => {
                    *has_tool_calls = true;
                    break;
                }
                LlmChunk::Done(_) => break,
            }
        }
        Ok(())
    }

    match &provider {
        ProviderKind::OpenAi { key, model, base_url } => {
            let p = OpenAiProvider::new(key.clone(), model.clone(), base_url.clone());
            stream_turn(&p, &history, &tools, &mut streamed_text, &mut has_tool_calls).await?;
        }
        ProviderKind::Ollama { model, base_url } => {
            let p = OllamaProvider::new(model.clone(), base_url.clone());
            stream_turn(&p, &history, &tools, &mut streamed_text, &mut has_tool_calls).await?;
        }
    }

    let final_reply = if has_tool_calls {
        println!();
        let reply = match &provider {
            ProviderKind::OpenAi { key, model, base_url } => {
                let p = OpenAiProvider::new(key.clone(), model.clone(), base_url.clone());
                let agent = AgentLoop::new(&p, &registry, max_steps);
                agent.run(&mut history).await?
            }
            ProviderKind::Ollama { model, base_url } => {
                let p = OllamaProvider::new(model.clone(), base_url.clone());
                let agent = AgentLoop::new(&p, &registry, max_steps);
                agent.run(&mut history).await?
            }
        };
        println!("{reply}");
        reply
    } else {
        println!();
        streamed_text
    };

    if !no_memory {
        let summary = format!("User: {message}\nAssistant: {final_reply}");
        try_store_memory(&cfg, &summary).await;
    }

    Ok(())
}
