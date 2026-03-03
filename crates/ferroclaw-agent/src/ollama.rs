use async_trait::async_trait;
use ferroclaw_core::{FerroError, Message};
use serde_json::Value;
use tokio_stream::Stream;

use crate::openai::OpenAiProvider;
use crate::provider::{LlmChunk, LlmProvider, LlmResponse};

/// Ollama uses the same OpenAI-compatible API, just a different base URL.
pub struct OllamaProvider {
    inner: OpenAiProvider,
}

impl OllamaProvider {
    /// `base_url` defaults to http://localhost:11434/v1
    pub fn new(model: String, base_url: Option<String>) -> Self {
        let base_url = base_url.unwrap_or_else(|| "http://localhost:11434/v1".into());
        Self {
            // Ollama's OpenAI-compat endpoint doesn't require a real key
            inner: OpenAiProvider::new("ollama".into(), model, Some(base_url)),
        }
    }
}

#[async_trait]
impl LlmProvider for OllamaProvider {
    fn model_name(&self) -> &str {
        self.inner.model_name()
    }

    async fn complete(
        &self,
        messages: &[Message],
        tools: &[Value],
    ) -> Result<LlmResponse, FerroError> {
        self.inner.complete(messages, tools).await
    }

    fn complete_stream<'a>(
        &'a self,
        messages: &'a [Message],
        tools: &'a [Value],
    ) -> impl Stream<Item = Result<LlmChunk, FerroError>> + Send + 'a {
        self.inner.complete_stream(messages, tools)
    }
}
