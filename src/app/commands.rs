use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{ChatApp, ModelPicker, validate_agent};
use crate::{agent::ChatAgent, provider::factory, tui::slash};

impl ChatApp {
    pub(super) async fn handle_slash_command(&mut self, command: &str) {
        self.error = None;
        if let Err(error) = self.execute_slash_command(command).await {
            self.error = Some(error.to_string());
        }
    }

    pub(super) async fn execute_slash_command(&mut self, command: &str) -> Result<()> {
        let mut arguments = command.split_whitespace();
        let name = arguments.next().unwrap_or_default();
        match name {
            "/help" => self.add_system_message(slash::help_text()),
            "/status" => self.add_system_message(format!(
                "Provider：{}  模型：{}  推理显示：{}",
                self.agent.provider_id(),
                self.agent.model(),
                if self.show_reasoning { "开" } else { "关" }
            )),
            "/providers" => {
                self.add_system_message(format!(
                    "可用 Provider：{}",
                    factory::PROVIDER_IDS.join(", ")
                ));
            }
            "/provider" => {
                if let Some(provider_id) = arguments.next() {
                    anyhow::ensure!(arguments.next().is_none(), "用法：/provider <名称>");
                    self.switch_provider(provider_id).await?;
                } else {
                    self.add_system_message(format!(
                        "当前 Provider：{}；可用：{}",
                        self.agent.provider_id(),
                        factory::PROVIDER_IDS.join(", ")
                    ));
                }
            }
            "/models" => self.open_model_picker().await?,
            "/model" => {
                if let Some(model) = arguments.next() {
                    anyhow::ensure!(arguments.next().is_none(), "用法：/model <模型 ID>");
                    self.switch_model(model).await?;
                } else {
                    self.open_model_picker().await?;
                }
            }
            "/new" | "/clear" => {
                anyhow::ensure!(arguments.next().is_none(), "用法：{name}");
                self.open_session(self.store.create_session()?)?;
            }
            "/reasoning" => {
                let enabled = match arguments.next() {
                    Some("on") => true,
                    Some("off") => false,
                    _ => anyhow::bail!("用法：/reasoning on|off"),
                };
                anyhow::ensure!(arguments.next().is_none(), "用法：/reasoning on|off");
                self.show_reasoning = enabled;
                self.config.display.show_reasoning = enabled;
                self.config.save(&self.config_path)?;
                self.add_system_message(if enabled {
                    "已显示推理内容；推理内容不会保存。"
                } else {
                    "已隐藏推理内容。"
                });
            }
            _ => anyhow::bail!("未知命令“{name}”；输入 /help 查看可用命令"),
        }
        Ok(())
    }

    pub(super) async fn show_model_picker(&mut self) {
        self.error = None;
        if let Err(error) = self.open_model_picker().await {
            self.error = Some(error.to_string());
        }
    }

    pub(super) async fn open_model_picker(&mut self) -> Result<()> {
        let mut models = self
            .agent
            .models()
            .await?
            .into_iter()
            .map(|model| model.id)
            .collect::<Vec<_>>();
        models.sort();
        models.dedup();
        anyhow::ensure!(
            !models.is_empty(),
            "Provider“{}”当前没有可选模型",
            self.agent.provider_id()
        );
        let selected = models
            .iter()
            .position(|model| model == self.agent.model())
            .unwrap_or(0);
        self.model_picker = Some(ModelPicker { models, selected });
        self.input.clear();
        Ok(())
    }

    pub(super) async fn handle_model_picker_key(&mut self, key: KeyEvent) -> Result<bool> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Ok(false);
        }
        let Some(picker) = self.model_picker.as_mut() else {
            return Ok(true);
        };
        match key.code {
            KeyCode::Up => {
                picker.selected = picker
                    .selected
                    .checked_sub(1)
                    .unwrap_or(picker.models.len() - 1);
            }
            KeyCode::Down => picker.selected = (picker.selected + 1) % picker.models.len(),
            KeyCode::Enter => {
                let model = picker.models[picker.selected].clone();
                self.model_picker = None;
                if let Err(error) = self.switch_model(&model).await {
                    self.error = Some(error.to_string());
                }
            }
            KeyCode::Esc => self.model_picker = None,
            _ => {}
        }
        Ok(true)
    }

    async fn switch_provider(&mut self, provider_id: &str) -> Result<()> {
        let model = factory::default_model(provider_id)
            .ok_or_else(|| anyhow::anyhow!("未知 Provider“{provider_id}”"))?;
        let provider = factory::create(provider_id, &self.config, self.process_api_key.clone())?;
        let agent = ChatAgent::new(provider, model);
        validate_agent(&agent).await?;
        self.agent = agent;
        self.config.chat.provider = provider_id.to_owned();
        self.config.chat.model = model.to_owned();
        self.config.save(&self.config_path)?;
        self.add_system_message(format!("已切换到 Provider：{provider_id}，模型：{model}。"));
        Ok(())
    }

    async fn switch_model(&mut self, model: &str) -> Result<()> {
        let models = self.agent.models().await?;
        anyhow::ensure!(
            models.iter().any(|candidate| candidate.id == model),
            "模型“{model}”当前不可用；输入 /models 查看模型列表"
        );
        self.agent.set_model(model);
        self.config.chat.provider = self.agent.provider_id().to_owned();
        self.config.chat.model = model.to_owned();
        self.config.save(&self.config_path)?;
        self.add_system_message(format!("已切换到模型：{model}。"));
        Ok(())
    }
}
