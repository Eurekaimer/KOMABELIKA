mod agent;
mod app;
mod cli;
mod commands;
mod config;
mod credentials;
mod memory;
mod persona;
mod provider;
mod telemetry;
mod tui;

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use cli::{ChatArgs, Cli, Command};
use config::AppConfig;
use directories::ProjectDirs;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();
    let cli = Cli::parse();
    let project_dirs = project_dirs()?;
    let config_path = config_path(&project_dirs);
    let mut config = AppConfig::load(&config_path)?;
    init_logging(&config);
    let data_dir = data_dir(&project_dirs);

    match cli.command {
        None => {
            commands::chat::run(&mut config, &config_path, &data_dir, ChatArgs::default()).await
        }
        Some(Command::Chat(args)) => {
            commands::chat::run(&mut config, &config_path, &data_dir, args).await
        }
        Some(Command::Config(args)) => commands::configure::run(&mut config, &config_path, args),
        Some(Command::Models(args)) => commands::models::run(&config, args).await,
        Some(Command::Login(args)) => commands::credentials::login(args),
        Some(Command::Logout(args)) => commands::credentials::logout(args),
        Some(Command::Doctor) => commands::doctor::run(&config, &config_path, &data_dir).await,
    }
}

fn init_logging(config: &AppConfig) {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new(config.logging.level.clone())),
        )
        .with_writer(std::io::stderr)
        .init();
}

fn project_dirs() -> Result<ProjectDirs> {
    ProjectDirs::from("dev", "Komari Call", "komari-call")
        .context("could not determine the user configuration and data directories")
}

fn config_path(project_dirs: &ProjectDirs) -> PathBuf {
    std::env::var_os("KOMARI_CALL_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|| project_dirs.config_dir().join("config.toml"))
}

fn data_dir(project_dirs: &ProjectDirs) -> PathBuf {
    std::env::var_os("KOMARI_CALL_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| project_dirs.data_local_dir().to_path_buf())
}
