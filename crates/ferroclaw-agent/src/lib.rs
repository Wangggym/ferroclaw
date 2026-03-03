pub mod config;
pub mod loop_agent;
pub mod ollama;
pub mod openai;
pub mod provider;

pub use config::{AgentConfig, LlmBackend};
pub use loop_agent::AgentLoop;
pub use provider::LlmProvider;
