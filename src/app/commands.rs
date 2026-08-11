use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{ChatApp, ModelPicker, validate_agent};
use crate::{agent::ChatAgent, config::BorderStyle, provider::factory, tui::slash};

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
                self.provider_id(),
                self.model(),
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
                        self.provider_id(),
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
            "/login" => {
                let provider_id = arguments
                    .next()
                    .map(str::to_owned)
                    .unwrap_or_else(|| self.provider_id().to_owned());
                anyhow::ensure!(arguments.next().is_none(), "用法：/login [provider]");
                self.begin_credential_entry(&provider_id)?;
            }
            "/border" => {
                if let Some(style) = arguments.next() {
                    self.config.display.border_style = match style {
                        "plain" => BorderStyle::Plain,
                        "rounded" => BorderStyle::Rounded,
                        "double" => BorderStyle::Double,
                        "thick" => BorderStyle::Thick,
                        _ => anyhow::bail!("用法：/border plain|rounded|double|thick"),
                    };
                    anyhow::ensure!(
                        arguments.next().is_none(),
                        "用法：/border plain|rounded|double|thick"
                    );
                    self.config.save(&self.config_path)?;
                    self.add_system_message("边框样式已保存。");
                } else {
                    self.add_system_message(format!(
                        "当前边框：{}；可用：plain, rounded, double, thick",
                        self.config.display.border_style.as_str()
                    ));
                }
            }
            "/text" => {
                if let Some(weight) = arguments.next() {
                    self.config.display.bold_text = match weight {
                        "normal" => false,
                        "bold" => true,
                        _ => anyhow::bail!("用法：/text normal|bold"),
                    };
                    anyhow::ensure!(arguments.next().is_none(), "用法：/text normal|bold");
                    self.config.save(&self.config_path)?;
                    self.add_system_message("绿色正文粗细已保存。");
                } else {
                    self.add_system_message(format!(
                        "当前正文：绿色 {}；可用：normal, bold",
                        if self.config.display.bold_text {
                            "bold"
                        } else {
                            "normal"
                        }
                    ));
                }
            }
            "/new" | "/clear" => {
                anyhow::ensure!(arguments.next().is_none(), "用法：{name}");
                self.open_session(self.store.create_session()?)?;
            }
            "/reasoning" => {
                if let Some(value) = arguments.next() {
                    let enabled = match value {
                        "on" => true,
                        "off" => false,
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
                } else {
                    self.add_system_message(format!(
                        "推理显示：{}；可用：on, off",
                        if self.show_reasoning { "on" } else { "off" }
                    ));
                }
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
        let agent = self.active_agent()?;
        let mut models = agent
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
            agent.provider_id()
        );
        let selected = models
            .iter()
            .position(|model| model == agent.model())
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
        self.agent = Some(agent);
        self.config.chat.provider = provider_id.to_owned();
        self.config.chat.model = model.to_owned();
        self.config.save(&self.config_path)?;
        self.add_system_message(format!("已切换到 Provider：{provider_id}，模型：{model}。"));
        Ok(())
    }

    async fn switch_model(&mut self, model: &str) -> Result<()> {
        let agent = self.active_agent()?;
        let models = agent.models().await?;
        anyhow::ensure!(
            models.iter().any(|candidate| candidate.id == model),
            "模型“{model}”当前不可用；输入 /models 查看模型列表"
        );
        let provider_id = agent.provider_id().to_owned();
        self.agent
            .as_mut()
            .expect("active agent was checked above")
            .set_model(model);
        self.config.chat.provider = provider_id;
        self.config.chat.model = model.to_owned();
        self.config.save(&self.config_path)?;
        self.add_system_message(format!("已切换到模型：{model}。"));
        Ok(())
    }
}
