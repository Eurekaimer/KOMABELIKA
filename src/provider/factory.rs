use std::{sync::Arc, time::Duration};

use anyhow::{Context, Result};

use crate::{config::AppConfig, credentials::resolve_deepseek};

use super::{
    ChatProvider,
    deepseek::{DeepSeekProvider, DeepSeekSettings},
    mock::MockProvider,
};

pub const PROVIDER_IDS: [&str; 2] = ["deepseek", "mock"];

pub fn default_model(provider_id: &str) -> Option<&'static str> {
    match provider_id {
        "deepseek" => Some("deepseek-chat"),
        "mock" => Some("komari-mock"),
        _ => None,
    }
}

pub fn create(
    provider_id: &str,
    config: &AppConfig,
    process_api_key: Option<String>,
) -> Result<Arc<dyn ChatProvider>> {
    match provider_id {
        "mock" => Ok(Arc::new(MockProvider::default())),
        "deepseek" => create_deepseek(config, process_api_key),
        unknown => anyhow::bail!("unknown provider '{unknown}'"),
    }
}

fn create_deepseek(
    config: &AppConfig,
    process_api_key: Option<String>,
) -> Result<Arc<dyn ChatProvider>> {
    let settings = &config.providers.deepseek;
    anyhow::ensure!(settings.enabled, "DeepSeek is disabled in configuration");
    let credential = resolve_deepseek(process_api_key, &settings.api_key_env).context(
        "no DeepSeek API key found; run `komari-call login deepseek` or set DEEPSEEK_API_KEY",
    )?;
    let provider = DeepSeekProvider::new(
        DeepSeekSettings {
            base_url: settings.base_url.clone(),
            timeout: Duration::from_secs(settings.timeout_seconds),
            thinking: settings.thinking,
            max_tokens: settings.max_tokens,
        },
        credential.expose().to_owned(),
    )?;
    Ok(Arc::new(provider))
}
