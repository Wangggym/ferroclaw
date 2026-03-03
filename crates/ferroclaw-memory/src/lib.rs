pub mod embedding;
pub mod error;
pub mod manager;
pub mod search;

pub use embedding::{EmbeddingProvider, OpenAiEmbedding};
pub use error::MemoryError;
pub use manager::{retrieve_context, store_conversation_memory, MemoryEntry, MemoryManager};
pub use search::{apply_temporal_decay, cosine_similarity};
