use std::{sync::Arc, time::Duration};

use anyhow::Result;

use crate::{
    config::AppConfig,
    credentials::{resolve_deepseek, resolve_opencode_go},
};

use super::{
    ChatProvider,
    deepseek::{DeepSeekProvider, DeepSeekSettings},
    opencode_go::{OpenCodeGoProvider, OpenCodeGoSettings},
};

pub const PROVIDER_IDS: [&str; 2] = ["deepseek", "opencode-go"];

#[derive(Debug, thiserror::Error)]
pub enum MissingCredential {
    #[error("no DeepSeek API key found; run `komari-call login deepseek` or set DEEPSEEK_API_KEY")]
    DeepSeek,
    #[error(
        "no OpenCode Go API key found; run komari-call login opencode-go or set OPENCODE_API_KEY"
    )]
    OpenCodeGo,
}

pub fn default_model(provider_id: &str) -> Option<&'static str> {
    match provider_id {
        "deepseek" => Some("deepseek-v4-flash"),
        "opencode-go" => Some("deepseek-v4-flash"),
        _ => None,
    }
}

pub fn create(
    provider_id: &str,
    config: &AppConfig,
    process_api_key: Option<String>,
) -> Result<Arc<dyn ChatProvider>> {
    match provider_id {
        "deepseek" => create_deepseek(config, process_api_key),
        "opencode-go" => create_opencode_go(config, process_api_key),
        unknown => anyhow::bail!("unknown provider '{unknown}'"),
    }
}

fn create_deepseek(
    config: &AppConfig,
    process_api_key: Option<String>,
) -> Result<Arc<dyn ChatProvider>> {
    let settings = &config.providers.deepseek;
    anyhow::ensure!(settings.enabled, "DeepSeek is disabled in configuration");
    let credential = resolve_deepseek(process_api_key, &settings.api_key_env)
        .ok_or(MissingCredential::DeepSeek)?;
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

fn create_opencode_go(
    config: &AppConfig,
    process_api_key: Option<String>,
) -> Result<Arc<dyn ChatProvider>> {
    let settings = &config.providers.opencode_go;
    anyhow::ensure!(settings.enabled, "OpenCode Go is disabled in configuration");
    let credential = resolve_opencode_go(process_api_key, &settings.api_key_env)
        .ok_or(MissingCredential::OpenCodeGo)?;
    let provider = OpenCodeGoProvider::new(
        OpenCodeGoSettings {
            base_url: settings.base_url.clone(),
            timeout: Duration::from_secs(settings.timeout_seconds),
            max_tokens: settings.max_tokens,
        },
        credential.expose().to_owned(),
    )?;
    Ok(Arc::new(provider))
}
