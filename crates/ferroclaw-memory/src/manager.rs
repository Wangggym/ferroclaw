use crate::{
    embedding::EmbeddingProvider,
    error::MemoryError,
    search::{apply_temporal_decay, cosine_similarity, decode_embedding, encode_embedding},
};
use sqlx::{migrate::MigrateDatabase, sqlite::SqlitePool, Sqlite};
use tracing::info;
use uuid::Uuid;

const DEFAULT_DB_PATH: &str = "~/.local/share/ferroclaw/memory.db";

/// A single memory entry returned by a search.
#[derive(Debug, Clone)]
pub struct MemoryEntry {
    pub id: String,
    pub content: String,
    pub score: f32,
}

pub struct MemoryManager {
    pool: SqlitePool,
}

impl MemoryManager {
    /// Open (or create) the memory database at the given path.
    pub async fn open(db_url: &str) -> Result<Self, MemoryError> {
        let db_url = expand_tilde(db_url);

        if let Some(parent) = std::path::Path::new(&db_url).parent() {
            std::fs::create_dir_all(parent)?;
        }

        let sqlite_url = format!("sqlite:{db_url}");
        if !Sqlite::database_exists(&sqlite_url).await.unwrap_or(false) {
            Sqlite::create_database(&sqlite_url)
                .await
                .map_err(MemoryError::Database)?;
        }

        let pool = SqlitePool::connect(&sqlite_url)
            .await
            .map_err(MemoryError::Database)?;

        sqlx::migrate!("./migrations").run(&pool).await?;

        info!("memory database ready: {db_url}");
        Ok(Self { pool })
    }

    pub async fn open_default() -> Result<Self, MemoryError> {
        Self::open(DEFAULT_DB_PATH).await
    }

    /// Store a new memory entry.
    pub async fn store(&self, content: &str, embedding: &[f32]) -> Result<String, MemoryError> {
        let id = Uuid::new_v4().to_string();
        let blob = encode_embedding(embedding);

        sqlx::query("INSERT INTO memory_entries (id, content, embedding) VALUES (?, ?, ?)")
            .bind(&id)
            .bind(content)
            .bind(&blob)
            .execute(&self.pool)
            .await
            .map_err(MemoryError::Database)?;

        info!("stored memory entry: {id}");
        Ok(id)
    }

    /// Retrieve the top-k most relevant memory entries for a query embedding,
    /// applying temporal decay to scores.
    pub async fn search(
        &self,
        query_embedding: &[f32],
        top_k: usize,
    ) -> Result<Vec<MemoryEntry>, MemoryError> {
        let rows = sqlx::query_as::<_, (String, String, Vec<u8>, String)>(
            "SELECT id, content, embedding, accessed_at FROM memory_entries",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(MemoryError::Database)?;

        let now = chrono::Utc::now();
        let mut scored: Vec<(f32, String, String)> = rows
            .into_iter()
            .map(|(id, content, blob, accessed_at)| {
                let emb = decode_embedding(&blob);
                let sim = cosine_similarity(query_embedding, &emb);

                // Parse accessed_at as UTC
                let days = chrono::NaiveDateTime::parse_from_str(&accessed_at, "%Y-%m-%d %H:%M:%S")
                    .ok()
                    .map(|dt| {
                        let utc = dt.and_utc();
                        (now - utc).num_seconds() as f64 / 86400.0
                    })
                    .unwrap_or(0.0);

                let score = apply_temporal_decay(sim, days);
                (score, id, content)
            })
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        scored.truncate(top_k);

        // Update accessed_at for retrieved entries
        for (_, ref id, _) in &scored {
            let _ =
                sqlx::query("UPDATE memory_entries SET accessed_at = datetime('now') WHERE id = ?")
                    .bind(id)
                    .execute(&self.pool)
                    .await;
        }

        Ok(scored
            .into_iter()
            .map(|(score, id, content)| MemoryEntry { id, content, score })
            .collect())
    }

    /// List all memory entries (without embeddings).
    pub async fn list(&self) -> Result<Vec<(String, String, String)>, MemoryError> {
        let rows = sqlx::query_as::<_, (String, String, String)>(
            "SELECT id, content, created_at FROM memory_entries ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(MemoryError::Database)?;
        Ok(rows)
    }

    /// Delete a specific memory entry by ID.
    pub async fn forget(&self, id: &str) -> Result<bool, MemoryError> {
        let r = sqlx::query("DELETE FROM memory_entries WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(MemoryError::Database)?;
        Ok(r.rows_affected() > 0)
    }

    /// Delete all memory entries.
    pub async fn clear_all(&self) -> Result<u64, MemoryError> {
        let r = sqlx::query("DELETE FROM memory_entries")
            .execute(&self.pool)
            .await
            .map_err(MemoryError::Database)?;
        Ok(r.rows_affected())
    }
}

/// Store a memory extracted from a conversation turn, using the LLM summary
/// and an embedding provider.
pub async fn store_conversation_memory(
    manager: &MemoryManager,
    embedder: &dyn EmbeddingProvider,
    summary: &str,
) -> Result<String, MemoryError> {
    let embedding = embedder.embed(summary).await?;
    manager.store(summary, &embedding).await
}

/// Retrieve relevant memories for a query string and format them as context.
pub async fn retrieve_context(
    manager: &MemoryManager,
    embedder: &dyn EmbeddingProvider,
    query: &str,
    top_k: usize,
) -> Result<String, MemoryError> {
    let embedding = embedder.embed(query).await?;
    let entries = manager.search(&embedding, top_k).await?;

    if entries.is_empty() {
        return Ok(String::new());
    }

    let mut ctx = String::from("Relevant memories from previous conversations:\n");
    for (i, entry) in entries.iter().enumerate() {
        ctx.push_str(&format!("{}. {}\n", i + 1, entry.content));
    }
    Ok(ctx)
}

fn expand_tilde(path: &str) -> String {
    if path.starts_with("~/") {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        format!("{}{}", home, &path[1..])
    } else {
        path.to_owned()
    }
}
