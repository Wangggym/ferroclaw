use anyhow::Result;
use ferroclaw_agent::AgentConfig;
use std::io::{self, Write as IoWrite};

pub async fn run_onboard() -> Result<()> {
    println!("ferroclaw onboard — interactive setup");
    println!();

    let config_path = AgentConfig::default_config_path();
    println!("Config file: {}", config_path.display());
    println!();

    print!("Backend [openai/ollama] (default: openai): ");
    io::stdout().flush()?;
    let mut backend_str = String::new();
    io::stdin().read_line(&mut backend_str)?;
    let backend = backend_str.trim();
    let use_ollama = backend.eq_ignore_ascii_case("ollama");

    let (api_key, base_url) = if use_ollama {
        print!("Ollama base URL (default: http://localhost:11434): ");
        io::stdout().flush()?;
        let mut url = String::new();
        io::stdin().read_line(&mut url)?;
        let url = url.trim().to_owned();
        let base = if url.is_empty() { None } else { Some(url) };
        (None::<String>, base)
    } else {
        print!("OpenAI API key (sk-...): ");
        io::stdout().flush()?;
        let mut key = String::new();
        io::stdin().read_line(&mut key)?;
        let key = key.trim().to_owned();
        print!("OpenAI base URL (leave blank for default): ");
        io::stdout().flush()?;
        let mut url = String::new();
        io::stdin().read_line(&mut url)?;
        let url = url.trim().to_owned();
        let base = if url.is_empty() { None } else { Some(url) };
        (if key.is_empty() { None } else { Some(key) }, base)
    };

    let default_model = if use_ollama { "llama3" } else { "gpt-4o" };
    print!("Model (default: {default_model}): ");
    io::stdout().flush()?;
    let mut model_str = String::new();
    io::stdin().read_line(&mut model_str)?;
    let model = model_str.trim().to_owned();
    let model = if model.is_empty() { None } else { Some(model) };

    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut lines = Vec::new();
    lines.push(format!(
        "backend = \"{}\"",
        if use_ollama { "ollama" } else { "openai" }
    ));
    if let Some(m) = model {
        lines.push(format!("model = \"{m}\""));
    }
    if let Some(key) = api_key {
        lines.push(format!("openai_api_key = \"{key}\""));
    }
    if let Some(url) = base_url {
        if use_ollama {
            lines.push(format!("ollama_base_url = \"{url}\""));
        } else {
            lines.push(format!("openai_base_url = \"{url}\""));
        }
    }

    std::fs::write(&config_path, lines.join("\n") + "\n")?;

    // Restrict config file permissions to owner-only (API key protection)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&config_path, std::fs::Permissions::from_mode(0o600))?;
    }

    println!();
    println!("Config written to: {}", config_path.display());
    println!("Run: ferroclaw agent -m \"hello\"");
    Ok(())
}
