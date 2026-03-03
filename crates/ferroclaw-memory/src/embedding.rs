use async_trait::async_trait;
use crate::error::MemoryError;

pub const OPENAI_EMBEDDING_DIM: usize = 1536; // text-embedding-3-small

/// A provider that converts text into a fixed-length embedding vector.
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, MemoryError>;
    fn dim(&self) -> usize;
}

// ── OpenAI implementation ────────────────────────────────────────────────────

use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct EmbedRequest<'a> {
    input: &'a str,
    model: &'a str,
}

#[derive(Deserialize)]
struct EmbedResponse {
    data: Vec<EmbedData>,
}

#[derive(Deserialize)]
struct EmbedData {
    embedding: Vec<f32>,
}

pub struct OpenAiEmbedding {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
}

impl OpenAiEmbedding {
    pub fn new(api_key: String, base_url: Option<String>) -> Self {
        // Normalize base_url: strip trailing /v1 or /v1/ so we can append it ourselves
        let raw = base_url.unwrap_or_else(|| "https://api.openai.com".to_owned());
        let base = raw.trim_end_matches('/').trim_end_matches("/v1").to_owned();
        Self {
            client: Client::new(),
            api_key,
            base_url: base,
            model: "text-embedding-3-small".to_owned(),
        }
    }
}

#[async_trait]
impl EmbeddingProvider for OpenAiEmbedding {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, MemoryError> {
        let url = format!("{}/v1/embeddings", self.base_url);
        let resp: EmbedResponse = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&EmbedRequest {
                input: text,
                model: &self.model,
            })
            .send()
            .await
            .map_err(|e| MemoryError::Embedding(e.to_string()))?
            .error_for_status()
            .map_err(|e| MemoryError::Embedding(e.to_string()))?
            .json()
            .await
            .map_err(|e| MemoryError::Embedding(e.to_string()))?;

        resp.data
            .into_iter()
            .next()
            .map(|d| d.embedding)
            .ok_or_else(|| MemoryError::Embedding("empty embedding response".into()))
    }

    fn dim(&self) -> usize {
        OPENAI_EMBEDDING_DIM
    }
}
