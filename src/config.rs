use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct AppConfig {
    pub chat: ChatConfig,
    pub persona: PersonaConfig,
    pub memory: MemoryConfig,
    pub display: DisplayConfig,
    pub logging: LoggingConfig,
    pub providers: ProvidersConfig,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct ChatConfig {
    pub provider: String,
    pub model: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct PersonaConfig {
    pub profile: String,
    pub language: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct MemoryConfig {
    pub enabled: bool,
    pub max_retrieved: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct DisplayConfig {
    pub show_reasoning: bool,
    pub bold_text: bool,
    pub border_style: BorderStyle,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BorderStyle {
    Plain,
    Rounded,
    Double,
    Thick,
}

impl Default for BorderStyle {
    fn default() -> Self {
        Self::Thick
    }
}

impl BorderStyle {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::Rounded => "rounded",
            Self::Double => "double",
            Self::Thick => "thick",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct LoggingConfig {
    pub level: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ProvidersConfig {
    pub deepseek: DeepSeekConfig,
    pub opencode_go: OpenCodeGoConfig,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct DeepSeekConfig {
    pub enabled: bool,
    pub base_url: String,
    pub api_key_env: String,
    pub timeout_seconds: u64,
    pub thinking: bool,
    pub max_tokens: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct OpenCodeGoConfig {
    pub enabled: bool,
    pub base_url: String,
    pub api_key_env: String,
    pub timeout_seconds: u64,
    pub max_tokens: Option<u32>,
}

impl Default for ChatConfig {
    fn default() -> Self {
        Self {
            provider: "deepseek".into(),
            model: "deepseek-v4-flash".into(),
        }
    }
}

impl Default for PersonaConfig {
    fn default() -> Self {
        Self {
            profile: "komari".into(),
            language: "zh-CN".into(),
        }
    }
}
impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            show_reasoning: false,
            bold_text: true,
            border_style: BorderStyle::Thick,
        }
    }
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_retrieved: 5,
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "warn".into(),
        }
    }
}

impl Default for DeepSeekConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            base_url: "https://api.deepseek.com".into(),
            api_key_env: "DEEPSEEK_API_KEY".into(),
            timeout_seconds: 120,
            thinking: false,
            max_tokens: None,
        }
    }
}

impl Default for OpenCodeGoConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            base_url: "https://opencode.ai/zen/go/v1".into(),
            api_key_env: "OPENCODE_API_KEY".into(),
            timeout_seconds: 120,
            max_tokens: None,
        }
    }
}

impl AppConfig {
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(content) => toml::from_str(&content)
                .with_context(|| format!("failed to parse {}", path.display())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error)
                .with_context(|| format!("failed to read configuration {}", path.display())),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let parent = path.parent().context("configuration path has no parent")?;
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
        let content = toml::to_string_pretty(self).context("failed to serialize configuration")?;
        let temporary = path.with_extension("toml.tmp");
        std::fs::write(&temporary, content)
            .with_context(|| format!("failed to write {}", temporary.display()))?;
        std::fs::rename(&temporary, path)
            .with_context(|| format!("failed to replace {}", path.display()))?;
        Ok(())
    }

    pub fn formatted(&self) -> Result<String> {
        toml::to_string_pretty(self).context("failed to serialize configuration")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_partial_configuration_with_defaults() {
        let config: AppConfig = toml::from_str(
            r#"
                [memory]
                enabled = false
            "#,
        )
        .unwrap();

        assert_eq!(config.chat.provider, "deepseek");
        assert_eq!(config.chat.model, "deepseek-v4-flash");
        assert!(!config.memory.enabled);
        assert_eq!(config.memory.max_retrieved, 5);
        assert!(config.providers.opencode_go.enabled);
        assert_eq!(
            config.providers.opencode_go.base_url,
            "https://opencode.ai/zen/go/v1"
        );
        assert_eq!(config.providers.opencode_go.api_key_env, "OPENCODE_API_KEY");
        assert_eq!(config.providers.opencode_go.timeout_seconds, 120);
        assert_eq!(config.providers.opencode_go.max_tokens, None);
        assert!(config.display.bold_text);
        assert_eq!(config.display.border_style, BorderStyle::Thick);
    }

    #[test]
    fn saves_and_loads_configuration_atomically() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("komari-call/config.toml");
        let mut config = AppConfig::default();
        config.memory.enabled = false;
        config.logging.level = "info".into();
        config.save(&path).unwrap();

        let loaded = AppConfig::load(&path).unwrap();
        assert!(!loaded.memory.enabled);
        assert_eq!(loaded.logging.level, "info");
        assert!(!path.with_extension("toml.tmp").exists());
        assert!(loaded.display.bold_text);
        assert_eq!(loaded.display.border_style, BorderStyle::Thick);
        assert!(loaded.providers.opencode_go.enabled);
        assert_eq!(
            loaded.providers.opencode_go.base_url,
            "https://opencode.ai/zen/go/v1"
        );
        let formatted = loaded.formatted().unwrap();
        let reparsed: AppConfig = toml::from_str(&formatted).unwrap();
        assert_eq!(
            reparsed.providers.opencode_go.api_key_env,
            "OPENCODE_API_KEY"
        );
    }
}
