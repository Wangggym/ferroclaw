use ferroclaw_core::{FerroError, Message, MessageContent, Role, SessionId};
use sqlx::{migrate::MigrateDatabase, sqlite::SqlitePool, Sqlite};
use tracing::info;
use uuid::Uuid;

const DEFAULT_DB_PATH: &str = "~/.local/share/ferroclaw/sessions.db";

pub struct SessionManager {
    pool: SqlitePool,
}

impl SessionManager {
    /// Open (or create) the session database at the given path.
    pub async fn open(db_url: &str) -> Result<Self, FerroError> {
        // Expand tilde
        let db_url = if db_url.starts_with("~/") {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
            format!("{}{}", home, &db_url[1..])
        } else {
            db_url.to_owned()
        };

        // Ensure parent directory exists
        if let Some(parent) = std::path::Path::new(&db_url).parent() {
            std::fs::create_dir_all(parent).map_err(FerroError::Io)?;
        }

        let sqlite_url = format!("sqlite:{db_url}");
        if !Sqlite::database_exists(&sqlite_url).await.unwrap_or(false) {
            Sqlite::create_database(&sqlite_url)
                .await
                .map_err(|e| FerroError::Session(e.to_string()))?;
        }

        let pool = SqlitePool::connect(&sqlite_url)
            .await
            .map_err(|e| FerroError::Session(e.to_string()))?;

        // Run embedded migrations
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .map_err(|e| FerroError::Session(e.to_string()))?;

        info!("session database ready: {db_url}");
        Ok(Self { pool })
    }

    pub async fn open_default() -> Result<Self, FerroError> {
        Self::open(DEFAULT_DB_PATH).await
    }

    /// Create a new session and return its ID.
    pub async fn create_session(&self) -> Result<SessionId, FerroError> {
        let id = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO sessions (id) VALUES (?)")
            .bind(&id)
            .execute(&self.pool)
            .await
            .map_err(|e| FerroError::Session(e.to_string()))?;
        Ok(SessionId::from_string(id))
    }

    /// Append a message to an existing session.
    pub async fn append_message(
        &self,
        session_id: &SessionId,
        msg: &Message,
    ) -> Result<(), FerroError> {
        let role = match msg.role {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::Tool => "tool",
        };
        let content = serde_json::to_string(&msg.content).map_err(FerroError::Serialization)?;

        sqlx::query(
            "INSERT INTO messages (session_id, role, content) VALUES (?, ?, ?)",
        )
        .bind(session_id.as_str())
        .bind(role)
        .bind(&content)
        .execute(&self.pool)
        .await
        .map_err(|e| FerroError::Session(e.to_string()))?;

        // Update session timestamp
        sqlx::query("UPDATE sessions SET updated_at = datetime('now') WHERE id = ?")
            .bind(session_id.as_str())
            .execute(&self.pool)
            .await
            .map_err(|e| FerroError::Session(e.to_string()))?;

        Ok(())
    }

    /// Load all messages for a session.
    pub async fn load_history(
        &self,
        session_id: &SessionId,
    ) -> Result<Vec<Message>, FerroError> {
        let rows = sqlx::query_as::<_, (String, String)>(
            "SELECT role, content FROM messages WHERE session_id = ? ORDER BY id ASC",
        )
        .bind(session_id.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| FerroError::Session(e.to_string()))?;

        let mut messages = Vec::new();
        for (role_str, content_str) in rows {
            let role = match role_str.as_str() {
                "system" => Role::System,
                "user" => Role::User,
                "assistant" => Role::Assistant,
                "tool" => Role::Tool,
                other => {
                    return Err(FerroError::Session(format!(
                        "unknown role in db: {other}"
                    )))
                }
            };
            let content: MessageContent = serde_json::from_str(&content_str)
                .map_err(FerroError::Serialization)?;
            messages.push(Message { role, content });
        }

        Ok(messages)
    }

    /// List all sessions (id, title, updated_at).
    pub async fn list_sessions(&self) -> Result<Vec<(String, Option<String>, String)>, FerroError> {
        let rows = sqlx::query_as::<_, (String, Option<String>, String)>(
            "SELECT id, title, updated_at FROM sessions ORDER BY updated_at DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| FerroError::Session(e.to_string()))?;
        Ok(rows)
    }

    /// Delete all messages and the session record.
    pub async fn clear_session(&self, session_id: &SessionId) -> Result<(), FerroError> {
        sqlx::query("DELETE FROM sessions WHERE id = ?")
            .bind(session_id.as_str())
            .execute(&self.pool)
            .await
            .map_err(|e| FerroError::Session(e.to_string()))?;
        Ok(())
    }

    /// Delete ALL sessions.
    pub async fn clear_all(&self) -> Result<u64, FerroError> {
        let r = sqlx::query("DELETE FROM sessions")
            .execute(&self.pool)
            .await
            .map_err(|e| FerroError::Session(e.to_string()))?;
        Ok(r.rows_affected())
    }
}
