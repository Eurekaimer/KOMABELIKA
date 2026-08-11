use std::path::Path;

use anyhow::{Context, Result};

use crate::{config::AppConfig, credentials, memory::Store, provider::factory};

pub async fn run(config: &AppConfig, config_path: &Path, data_dir: &Path) -> Result<()> {
    println!("[ok] configuration: {}", config_path.display());
    std::fs::create_dir_all(data_dir)
        .with_context(|| format!("failed to create {}", data_dir.display()))?;
    Store::open(data_dir.join("komari-call.sqlite3"))?;
    println!(
        "[ok] database: {}",
        data_dir.join("komari-call.sqlite3").display()
    );

    match credentials::resolve_deepseek(None, &config.providers.deepseek.api_key_env) {
        Some(credential) => println!("[ok] DeepSeek credential: {}", credential.source),
        None => println!("[--] DeepSeek credential: not configured"),
    }
    match credentials::resolve_opencode_go(None, &config.providers.opencode_go.api_key_env) {
        Some(credential) => println!("[ok] OpenCode Go credential: {}", credential.source),
        None => println!("[--] OpenCode Go credential: not configured"),
    }

    let provider = factory::create(&config.chat.provider, config, None)?;
    provider.health_check().await?;
    let models = provider.list_models().await?;
    anyhow::ensure!(
        models.iter().any(|model| model.id == config.chat.model),
        "configured model '{}' is unavailable from {}",
        config.chat.model,
        config.chat.provider
    );
    println!(
        "[ok] current provider/model: {}/{}",
        config.chat.provider, config.chat.model
    );
    Ok(())
}
