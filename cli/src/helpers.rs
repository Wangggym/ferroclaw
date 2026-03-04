use anyhow::{Context, Result};
use ferroclaw_agent::{AgentConfig, LlmBackend};
use ferroclaw_memory::{
    retrieve_context, store_conversation_memory, MemoryManager, OpenAiEmbedding,
};
use ferroclaw_tools::ToolRegistry;

pub fn tool_names_str(registry: &ToolRegistry) -> String {
    registry
        .schemas()
        .iter()
        .map(|s| s["function"]["name"].as_str().unwrap_or("").to_owned())
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn build_system_prompt(tool_names: &str, memory_ctx: &str) -> String {
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

pub fn require_openai_key(cfg: &AgentConfig) -> Result<String> {
    cfg.openai_api_key.clone().context(
        "OpenAI API key not set. Add FERROCLAW_OPENAI_API_KEY to .env or run `ferroclaw onboard`.",
    )
}

pub fn make_embedder(cfg: &AgentConfig) -> Result<OpenAiEmbedding> {
    let key = require_openai_key(cfg)?;
    Ok(OpenAiEmbedding::new(key, cfg.openai_base_url.clone()))
}

pub async fn build_memory_context(cfg: &AgentConfig, query: &str) -> Result<String> {
    if cfg.openai_api_key.is_none() {
        return Ok(String::new());
    }
    let mm = MemoryManager::open_default().await?;
    let embedder = make_embedder(cfg)?;
    let ctx = retrieve_context(&mm, &embedder, query, 5).await?;
    Ok(ctx)
}

pub async fn try_store_memory(cfg: &AgentConfig, summary: &str) {
    if cfg.openai_api_key.is_none() {
        return;
    }
    if let Ok(mm) = MemoryManager::open_default().await {
        if let Ok(embedder) = make_embedder(cfg) {
            let _ = store_conversation_memory(&mm, &embedder, summary).await;
        }
    }
}

/// Build an enum-dispatched provider runner to avoid duplicating the match in every command.
pub enum ProviderKind {
    OpenAi {
        key: String,
        model: String,
        base_url: Option<String>,
    },
    Ollama {
        model: String,
        base_url: Option<String>,
    },
}

impl ProviderKind {
    pub fn from_config(cfg: &AgentConfig) -> Result<Self> {
        let model = cfg.model_name().to_owned();
        match cfg.backend {
            LlmBackend::OpenAi => {
                let key = require_openai_key(cfg)?;
                Ok(Self::OpenAi {
                    key,
                    model,
                    base_url: cfg.openai_base_url.clone(),
                })
            }
            LlmBackend::Ollama => Ok(Self::Ollama {
                model,
                base_url: cfg.ollama_base_url.clone(),
            }),
        }
    }
}
