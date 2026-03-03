use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Which LLM backend to use.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LlmBackend {
    #[default]
    OpenAi,
    Ollama,
}

/// Per-model call options.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallOptions {
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
}

impl Default for CallOptions {
    fn default() -> Self {
        Self {
            max_tokens: Some(4096),
            temperature: Some(0.7),
        }
    }
}

/// Top-level agent config (loaded from ~/.config/ferroclaw/config.toml).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentConfig {
    pub backend: LlmBackend,
    pub model: Option<String>,
    pub openai_api_key: Option<String>,
    pub openai_base_url: Option<String>,
    pub ollama_base_url: Option<String>,
    pub max_steps: Option<usize>,
}

impl AgentConfig {
    pub fn model_name(&self) -> &str {
        self.model
            .as_deref()
            .unwrap_or_else(|| match self.backend {
                LlmBackend::OpenAi => "gpt-4o",
                LlmBackend::Ollama => "llama3",
            })
    }

    pub fn max_steps(&self) -> usize {
        self.max_steps.unwrap_or(20)
    }

    /// Load config from ~/.config/ferroclaw/config.toml, with env var overrides.
    pub fn load() -> anyhow::Result<Self> {
        // Load .env if present (dotenvy)
        let _ = dotenvy::dotenv();

        let config_path = Self::default_config_path();
        let mut cfg: AgentConfig = if config_path.exists() {
            let raw = std::fs::read_to_string(&config_path)?;
            toml::from_str(&raw)?
        } else {
            AgentConfig::default()
        };

        // Env var overrides
        if let Ok(key) = std::env::var("FERROCLAW_OPENAI_API_KEY") {
            cfg.openai_api_key = Some(key);
        }
        if let Ok(url) = std::env::var("FERROCLAW_OPENAI_BASE_URL") {
            cfg.openai_base_url = Some(url);
        }
        if let Ok(url) = std::env::var("FERROCLAW_OLLAMA_BASE_URL") {
            cfg.ollama_base_url = Some(url);
        }
        if let Ok(model) = std::env::var("FERROCLAW_MODEL") {
            cfg.model = Some(model);
        }
        if let Ok(backend) = std::env::var("FERROCLAW_BACKEND") {
            cfg.backend = match backend.to_lowercase().as_str() {
                "ollama" => LlmBackend::Ollama,
                _ => LlmBackend::OpenAi,
            };
        }

        Ok(cfg)
    }

    pub fn default_config_path() -> PathBuf {
        dirs_next::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ferroclaw")
            .join("config.toml")
    }
}
