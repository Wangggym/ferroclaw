use async_trait::async_trait;
use ferroclaw_core::{FerroError, Message, ToolCall};
use serde_json::Value;
use tokio_stream::Stream;

/// A streaming token chunk from the LLM.
#[derive(Debug, Clone)]
pub enum LlmChunk {
    /// A delta of text content.
    Text(String),
    /// One or more tool calls requested by the model.
    ToolCalls(Vec<ToolCall>),
    /// Stream is done; optionally carries stop reason.
    Done(Option<String>),
}

/// The complete (non-streaming) response from an LLM call.
#[derive(Debug)]
pub struct LlmResponse {
    pub text: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub stop_reason: Option<String>,
}

impl LlmResponse {
    pub fn is_tool_call(&self) -> bool {
        !self.tool_calls.is_empty()
    }
}

/// Trait all LLM backends must implement.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Model identifier string.
    fn model_name(&self) -> &str;

    /// Non-streaming completion (collects the full response).
    async fn complete(
        &self,
        messages: &[Message],
        tools: &[Value],
    ) -> Result<LlmResponse, FerroError>;

    /// Streaming completion.
    fn complete_stream<'a>(
        &'a self,
        messages: &'a [Message],
        tools: &'a [Value],
    ) -> impl Stream<Item = Result<LlmChunk, FerroError>> + Send + 'a;
}
