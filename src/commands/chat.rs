use std::path::Path;

use anyhow::{Context, Result};

use crate::{
    agent::ChatAgent, app, cli::ChatArgs, config::AppConfig, memory::Store, provider::factory,
};

pub async fn run(config: &AppConfig, data_dir: &Path, args: ChatArgs) -> Result<()> {
    let provider_id = args.provider.map_or_else(
        || config.chat.provider.clone(),
        |provider| provider.to_string(),
    );
    let model = args.model.unwrap_or_else(|| {
        if provider_id == config.chat.provider {
            config.chat.model.clone()
        } else if provider_id == "deepseek" {
            "deepseek-v4-flash".into()
        } else {
            "komari-mock".into()
        }
    });
    let provider = factory::create(&provider_id, config, args.api_key)?;
    std::fs::create_dir_all(data_dir)
        .with_context(|| format!("failed to create {}", data_dir.display()))?;
    let store = Store::open(data_dir.join("komari-call.sqlite3"))?;
    let agent = ChatAgent::new(provider, model);
    app::run(agent, store, config.display.show_reasoning).await
}
