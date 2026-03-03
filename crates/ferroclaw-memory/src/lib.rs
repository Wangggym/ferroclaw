pub mod embedding;
pub mod error;
pub mod manager;
pub mod search;

pub use embedding::{EmbeddingProvider, OpenAiEmbedding};
pub use error::MemoryError;
pub use manager::{MemoryEntry, MemoryManager, retrieve_context, store_conversation_memory};
pub use search::{apply_temporal_decay, cosine_similarity};
