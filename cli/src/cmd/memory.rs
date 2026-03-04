use anyhow::Result;
use ferroclaw_agent::AgentConfig;
use ferroclaw_memory::{EmbeddingProvider, MemoryManager};

use crate::helpers::make_embedder;

#[derive(clap::Subcommand)]
pub enum MemoryCommands {
    /// List all memory entries
    List,
    /// Search memory for a query
    Search {
        /// The query to search for
        query: String,
        /// Number of results
        #[arg(short, long, default_value = "5")]
        top_k: usize,
    },
    /// Forget (delete) a memory entry by ID
    Forget { id: String },
    /// Clear all memory entries
    Clear,
}

pub async fn run_memory(action: MemoryCommands) -> Result<()> {
    let mm = MemoryManager::open_default().await?;
    match action {
        MemoryCommands::List => {
            let entries = mm.list().await?;
            if entries.is_empty() {
                println!("No memory entries.");
            } else {
                println!("{:<36}  {:<20}  Content", "ID", "Created");
                println!("{}", "-".repeat(80));
                for (id, content, created) in entries {
                    let preview = if content.chars().count() > 40 {
                        let cut: String = content.chars().take(40).collect();
                        format!("{cut}…")
                    } else {
                        content.clone()
                    };
                    println!("{id:<36}  {created:<20}  {preview}");
                }
            }
        }
        MemoryCommands::Search { query, top_k } => {
            let cfg = AgentConfig::load()?;
            let embedder = make_embedder(&cfg)?;
            let emb: Vec<f32> = embedder.embed(&query).await?;
            let results = mm.search(&emb, top_k).await?;
            if results.is_empty() {
                println!("No relevant memories found.");
            } else {
                for (i, entry) in results.iter().enumerate() {
                    println!("{}. [score: {:.3}] [id: {}]", i + 1, entry.score, entry.id);
                    println!("   {}", entry.content);
                    println!();
                }
            }
        }
        MemoryCommands::Forget { id } => {
            let deleted = mm.forget(&id).await?;
            if deleted {
                println!("Forgotten: {id}");
            } else {
                println!("Not found: {id}");
            }
        }
        MemoryCommands::Clear => {
            let n = mm.clear_all().await?;
            println!("Cleared {n} memory entry/entries.");
        }
    }
    Ok(())
}
