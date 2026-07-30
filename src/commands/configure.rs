use std::path::Path;

use anyhow::Result;

use crate::{cli::ConfigArgs, config::AppConfig};

pub fn run(config: &mut AppConfig, path: &Path, args: ConfigArgs) -> Result<()> {
    let changed = args.has_changes();
    if let Some(provider) = args.provider {
        config.chat.provider = provider.to_string();
    }
    if let Some(model) = args.model {
        config.chat.model = model;
    }
    if let Some(base_url) = args.deepseek_base_url {
        config.providers.deepseek.base_url = base_url;
    }
    if let Some(environment) = args.deepseek_api_key_env {
        config.providers.deepseek.api_key_env = environment;
    }
    if let Some(timeout) = args.deepseek_timeout {
        config.providers.deepseek.timeout_seconds = timeout;
    }
    if let Some(thinking) = args.deepseek_thinking {
        config.providers.deepseek.thinking = thinking;
    }
    if let Some(max_tokens) = args.deepseek_max_tokens {
        config.providers.deepseek.max_tokens = (max_tokens != 0).then_some(max_tokens);
    }
    if let Some(enabled) = args.memory {
        config.memory.enabled = enabled;
    }
    if let Some(max_retrieved) = args.max_retrieved {
        config.memory.max_retrieved = max_retrieved;
    }
    if let Some(show_reasoning) = args.show_reasoning {
        config.display.show_reasoning = show_reasoning;
    }
    if let Some(level) = args.log_level {
        config.logging.level = level;
    }
    if changed {
        config.save(path)?;
        println!("Updated {}", path.display());
    } else {
        println!("Configuration: {}", path.display());
    }
    print!("{}", config.formatted()?);
    Ok(())
}
