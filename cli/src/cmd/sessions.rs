use anyhow::Result;
use ferroclaw_session::SessionManager;

#[derive(clap::Subcommand)]
pub enum SessionCommands {
    /// List all sessions
    List,
    /// Clear all sessions
    Clear,
}

pub async fn run_sessions(action: SessionCommands) -> Result<()> {
    let sm = SessionManager::open_default().await?;
    match action {
        SessionCommands::List => {
            let sessions = sm.list_sessions().await?;
            if sessions.is_empty() {
                println!("No sessions.");
            } else {
                println!("{:<36}  {:<20}  Title", "ID", "Updated");
                println!("{}", "-".repeat(70));
                for (id, title, updated) in sessions {
                    println!(
                        "{:<36}  {:<20}  {}",
                        id,
                        updated,
                        title.as_deref().unwrap_or("-")
                    );
                }
            }
        }
        SessionCommands::Clear => {
            let n = sm.clear_all().await?;
            println!("Cleared {n} session(s).");
        }
    }
    Ok(())
}
