use anyhow::Result;

use crate::{cli::ModelsArgs, config::AppConfig, provider::factory};

pub async fn run(config: &AppConfig, args: ModelsArgs) -> Result<()> {
    let provider_id = args.provider.map_or_else(
        || config.chat.provider.clone(),
        |provider| provider.to_string(),
    );
    let provider = factory::create(&provider_id, config, args.api_key)?;
    let models = provider.list_models().await?;
    println!("Provider: {provider_id}");
    for model in models {
        println!("{}", model.id);
    }
    Ok(())
}
