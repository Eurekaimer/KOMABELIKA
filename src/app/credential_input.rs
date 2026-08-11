use std::borrow::Cow;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{ChatApp, validate_agent};
use crate::{agent::ChatAgent, credentials, provider::factory};

/// Controls who owns `ChatApp::input`.
///
/// In credential mode the buffer is a secret: it must bypass slash completion,
/// render only through `display`, and be cleared on success, failure, or cancel.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) enum InputMode {
    #[default]
    Chat,
    Credential {
        provider_id: String,
    },
}

impl InputMode {
    pub(super) fn is_credential(&self) -> bool {
        matches!(self, Self::Credential { .. })
    }

    pub(super) fn title(&self) -> &str {
        match self {
            Self::Chat => " 输入 ",
            Self::Credential { .. } => " API Key（隐藏输入） ",
        }
    }

    pub(super) fn display<'a>(&self, input: &'a str) -> Cow<'a, str> {
        match self {
            Self::Chat => Cow::Borrowed(input),
            Self::Credential { .. } => Cow::Owned("•".repeat(input.chars().count())),
        }
    }
}

impl ChatApp {
    pub(super) fn begin_credential_entry(&mut self, provider_id: &str) -> Result<()> {
        anyhow::ensure!(
            factory::PROVIDER_IDS.contains(&provider_id),
            "未知 Provider“{provider_id}”"
        );
        anyhow::ensure!(self.stream.is_none(), "请等待当前回复完成后再登录");
        self.input.clear();
        self.completion_index = 0;
        self.input_mode = InputMode::Credential {
            provider_id: provider_id.to_owned(),
        };
        self.error = None;
        Ok(())
    }

    pub(super) async fn handle_credential_key(&mut self, key: KeyEvent) -> Result<bool> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Ok(false);
        }
        match key.code {
            KeyCode::Esc => {
                self.input.clear();
                self.input_mode = InputMode::Chat;
                self.error = None;
            }
            KeyCode::Enter | KeyCode::Char('m')
                if key.code == KeyCode::Enter || key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                if let Err(error) = self.complete_credential_entry().await {
                    self.input.clear();
                    self.error = Some(error.to_string());
                }
            }
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Char(character) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.input.push(character);
            }
            _ => {}
        }
        Ok(true)
    }

    async fn complete_credential_entry(&mut self) -> Result<()> {
        let InputMode::Credential { provider_id } = &self.input_mode else {
            return Ok(());
        };
        let provider_id = provider_id.clone();
        let api_key = self.input.trim().to_owned();
        anyhow::ensure!(!api_key.is_empty(), "API Key 不能为空");

        let model = if provider_id == self.provider_id() {
            self.model()
        } else {
            factory::default_model(&provider_id)
                .ok_or_else(|| anyhow::anyhow!("未知 Provider“{provider_id}”"))?
        }
        .to_owned();
        let provider = factory::create(&provider_id, &self.config, Some(api_key.clone()))?;
        let agent = ChatAgent::new(provider, &model);
        // Validate before persisting so a mistyped secret never replaces a working key.
        validate_agent(&agent).await?;
        store_credential(&provider_id, &api_key)?;

        self.agent = Some(agent);
        self.process_api_key = None;
        self.config.chat.provider = provider_id.clone();
        self.config.chat.model = model.clone();
        self.config.save(&self.config_path)?;
        self.input.clear();
        self.input_mode = InputMode::Chat;
        self.error = None;
        self.add_system_message(format!(
            "已保存 {provider_id} API Key，并切换到模型 {model}。"
        ));
        Ok(())
    }
}

fn store_credential(provider_id: &str, api_key: &str) -> Result<()> {
    match provider_id {
        "deepseek" => credentials::store_deepseek(api_key),
        "opencode-go" => credentials::store_opencode_go(api_key),
        _ => anyhow::bail!("未知 Provider“{provider_id}”"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_display_never_exposes_the_secret() {
        let mode = InputMode::Credential {
            provider_id: "deepseek".into(),
        };
        let displayed = mode.display("sk-private");
        assert_eq!(displayed, "••••••••••");
        assert!(!displayed.contains("sk-private"));
    }
}
