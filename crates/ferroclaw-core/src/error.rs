use thiserror::Error;

#[derive(Debug, Error)]
pub enum FerroError {
    #[error("LLM provider error: {0}")]
    LlmProvider(String),

    #[error("Tool execution error: {tool} — {message}")]
    ToolExecution { tool: String, message: String },

    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    #[error("Session error: {0}")]
    Session(String),

    #[error("Memory error: {0}")]
    Memory(String),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Max steps exceeded ({0})")]
    MaxStepsExceeded(usize),

    #[error("{0}")]
    Other(String),
}
