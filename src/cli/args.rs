use std::fmt;

use clap::{Args, ValueEnum};

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum ProviderId {
    Deepseek,
    OpencodeGo,
}

impl fmt::Display for ProviderId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Deepseek => "deepseek",
            Self::OpencodeGo => "opencode-go",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum CredentialProvider {
    Deepseek,
    OpencodeGo,
}

#[derive(Args, Clone, Debug)]
pub struct CredentialArgs {
    /// Provider whose credential should be changed
    pub provider: CredentialProvider,
}

#[derive(Args, Clone, Debug, Default)]
pub struct ChatArgs {
    /// Override the configured provider for this chat
    #[arg(long)]
    pub provider: Option<ProviderId>,

    /// Override the configured model for this chat
    #[arg(long)]
    pub model: Option<String>,

    /// Selected provider key for this process only
    #[arg(long)]
    pub api_key: Option<String>,
}

#[derive(Args, Clone, Debug, Default)]
pub struct ModelsArgs {
    /// Provider whose models should be listed
    #[arg(long)]
    pub provider: Option<ProviderId>,

    /// Selected provider key for this process only
    #[arg(long)]
    pub api_key: Option<String>,
}

#[derive(Args, Clone, Debug, Default)]
pub struct ConfigArgs {
    /// Set the default chat provider
    #[arg(long)]
    pub provider: Option<ProviderId>,

    /// Set the default model identifier
    #[arg(long)]
    pub model: Option<String>,

    /// Set the DeepSeek API base URL
    #[arg(long)]
    pub deepseek_base_url: Option<String>,

    /// Set the fallback environment-variable name for the DeepSeek key
    #[arg(long)]
    pub deepseek_api_key_env: Option<String>,

    /// Set the DeepSeek request timeout in seconds
    #[arg(long, value_parser = parse_timeout)]
    pub deepseek_timeout: Option<u64>,

    /// Enable or disable DeepSeek thinking mode
    #[arg(long, value_parser = clap::value_parser!(bool))]
    pub deepseek_thinking: Option<bool>,

    /// Set or clear the DeepSeek output limit (0 clears it)
    #[arg(long)]
    pub deepseek_max_tokens: Option<u32>,

    /// Set the OpenCode Go API base URL
    #[arg(long)]
    pub opencode_go_base_url: Option<String>,

    /// Set the fallback environment-variable name for the OpenCode Go key
    #[arg(long)]
    pub opencode_go_api_key_env: Option<String>,

    /// Set the OpenCode Go request timeout in seconds
    #[arg(long, value_parser = parse_timeout)]
    pub opencode_go_timeout: Option<u64>,

    /// Set or clear the OpenCode Go output limit (0 clears it)
    #[arg(long)]
    pub opencode_go_max_tokens: Option<u32>,

    /// Enable or disable memory configuration
    #[arg(long, value_parser = clap::value_parser!(bool))]
    pub memory: Option<bool>,

    /// Set the maximum recalled memory count
    #[arg(long, value_parser = parse_retrieval_count)]
    pub max_retrieved: Option<usize>,

    /// Show or hide provider reasoning in the TUI
    #[arg(long, value_parser = clap::value_parser!(bool))]
    pub show_reasoning: Option<bool>,

    /// Set log verbosity
    #[arg(long, value_parser = ["error", "warn", "info", "debug", "trace"])]
    pub log_level: Option<String>,
}

impl ConfigArgs {
    pub fn has_changes(&self) -> bool {
        self.provider.is_some()
            || self.model.is_some()
            || self.deepseek_base_url.is_some()
            || self.deepseek_api_key_env.is_some()
            || self.deepseek_timeout.is_some()
            || self.deepseek_thinking.is_some()
            || self.deepseek_max_tokens.is_some()
            || self.opencode_go_base_url.is_some()
            || self.opencode_go_api_key_env.is_some()
            || self.opencode_go_timeout.is_some()
            || self.opencode_go_max_tokens.is_some()
            || self.memory.is_some()
            || self.max_retrieved.is_some()
            || self.show_reasoning.is_some()
            || self.log_level.is_some()
    }
}

fn parse_retrieval_count(value: &str) -> Result<usize, String> {
    parse_range(value, 1, 20, "memory retrieval count")
}

fn parse_timeout(value: &str) -> Result<u64, String> {
    parse_range(value, 5, 600, "timeout")
}

fn parse_range<T>(value: &str, minimum: T, maximum: T, name: &str) -> Result<T, String>
where
    T: std::str::FromStr + PartialOrd + Copy + fmt::Display,
{
    let parsed = value
        .parse::<T>()
        .map_err(|_| format!("{name} must be a number"))?;
    (parsed >= minimum && parsed <= maximum)
        .then_some(parsed)
        .ok_or_else(|| format!("{name} must be from {minimum} to {maximum}"))
}
