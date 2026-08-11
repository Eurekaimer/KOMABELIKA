use std::path::Path;

use anyhow::{Context, Result};

use crate::{
    agent::ChatAgent, app, cli::ChatArgs, config::AppConfig, memory::Store, provider::factory,
};

pub async fn run(
    config: &mut AppConfig,
    config_path: &Path,
    data_dir: &Path,
    args: ChatArgs,
) -> Result<()> {
    let provider_id = args.provider.map_or_else(
        || config.chat.provider.clone(),
        |provider| provider.to_string(),
    );
    let model = args.model.unwrap_or_else(|| {
        (provider_id == config.chat.provider)
            .then(|| config.chat.model.clone())
            .or_else(|| factory::default_model(&provider_id).map(str::to_owned))
            .expect("CLI providers must define a default model")
    });
    let process_api_key = args.api_key;
    let agent = match factory::create(&provider_id, config, process_api_key.clone()) {
        Ok(provider) => Some(ChatAgent::new(provider, model)),
        Err(error) if error.downcast_ref::<factory::MissingCredential>().is_some() => None,
        Err(error) => return Err(error),
    };
    std::fs::create_dir_all(data_dir)
        .with_context(|| format!("failed to create {}", data_dir.display()))?;
    let store = Store::open(data_dir.join("komari-call.sqlite3"))?;
    app::run(
        agent,
        store,
        config.clone(),
        config_path.to_path_buf(),
        process_api_key,
    )
    .await
}
