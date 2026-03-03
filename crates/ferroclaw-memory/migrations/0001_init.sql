-- ferroclaw-memory: long-term vector memory store
CREATE TABLE IF NOT EXISTS memory_entries (
    id          TEXT PRIMARY KEY NOT NULL,
    content     TEXT NOT NULL,
    embedding   BLOB NOT NULL,        -- f32 array serialized as little-endian bytes
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    accessed_at TEXT NOT NULL DEFAULT (datetime('now')),
    score       REAL NOT NULL DEFAULT 1.0
);

CREATE INDEX IF NOT EXISTS idx_memory_created  ON memory_entries (created_at DESC);
CREATE INDEX IF NOT EXISTS idx_memory_accessed ON memory_entries (accessed_at DESC);
