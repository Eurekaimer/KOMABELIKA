use std::io::{self, stdout};
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use anyhow::Result;
use crossterm::{
    event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures_util::StreamExt;
use ratatui::{Terminal, backend::CrosstermBackend};

use crate::{
    agent::ChatAgent,
    config::AppConfig,
    memory::{SessionSummary, Store, StoredMessage},
    provider::{ChatStream, Role, StreamEvent, TokenUsage, factory},
    tui::{
        chat::{self, ChatView, VisibleMessage},
        slash,
    },
};

pub async fn run(
    agent: ChatAgent,
    store: Store,
    config: AppConfig,
    config_path: PathBuf,
    process_api_key: Option<String>,
) -> Result<()> {
    validate_agent(&agent).await?;

    enable_raw_mode()?;
    let mut output = stdout();
    execute!(output, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(output);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let result = ChatApp::new(agent, store, config, config_path, process_api_key)?
        .run(&mut terminal)
        .await;
    let restore_result = restore_terminal(&mut terminal);
    result.and(restore_result)
}

async fn validate_agent(agent: &ChatAgent) -> Result<()> {
    anyhow::ensure!(
        agent.capabilities().streaming,
        "Provider '{}' 不支持流式输出",
        agent.provider_id()
    );
    let models = agent.models().await?;
    anyhow::ensure!(
        models.iter().any(|model| model.id == agent.model()),
        "Provider '{}' 当前没有模型 '{}'",
        agent.provider_id(),
        agent.model()
    );
    Ok(())
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

struct ChatApp {
    agent: ChatAgent,
    store: Store,
    config: AppConfig,
    config_path: PathBuf,
    process_api_key: Option<String>,
    session: SessionSummary,
    messages: Vec<StoredMessage>,
    input: String,
    completion_index: usize,
    stream: Option<ChatStream>,
    cancellation: Option<Arc<AtomicBool>>,
    streaming_text: String,
    reasoning_text: String,
    show_reasoning: bool,
    usage: TokenUsage,
    error: Option<String>,
}

impl ChatApp {
    fn new(
        agent: ChatAgent,
        store: Store,
        config: AppConfig,
        config_path: PathBuf,
        process_api_key: Option<String>,
    ) -> Result<Self> {
        let session = store.latest_or_create_session()?;
        let messages = store.load_messages(&session.id)?;
        Ok(Self {
            agent,
            store,
            show_reasoning: config.display.show_reasoning,
            config,
            config_path,
            process_api_key,
            session,
            messages,
            input: String::new(),
            completion_index: 0,
            stream: None,
            cancellation: None,
            streaming_text: String::new(),
            reasoning_text: String::new(),
            usage: TokenUsage::default(),
            error: None,
        })
    }

    async fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
        let mut events = EventStream::new();
        loop {
            self.draw(terminal)?;
            let next = if self.stream.is_some() {
                tokio::select! {
                    terminal_event = events.next() => LoopEvent::Terminal(terminal_event),
                    stream_event = next_stream_event(&mut self.stream) => LoopEvent::Stream(stream_event),
                }
            } else {
                LoopEvent::Terminal(events.next().await)
            };

            match next {
                LoopEvent::Terminal(Some(Ok(Event::Key(key))))
                    if key.kind == KeyEventKind::Press =>
                {
                    if !self.handle_key(key).await? {
                        break;
                    }
                }
                LoopEvent::Terminal(Some(Ok(_))) => {}
                LoopEvent::Terminal(Some(Err(error))) => return Err(error.into()),
                LoopEvent::Terminal(None) => break,
                LoopEvent::Stream(Some(event)) => self.handle_stream_event(event)?,
                LoopEvent::Stream(None) => {
                    self.error = Some("Provider 在完成前关闭了连接".into());
                    self.finish_generation(true)?;
                }
            }
        }
        if self.stream.is_some() {
            self.cancel_generation()?;
        }
        Ok(())
    }

    fn draw(&self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
        let messages = self
            .messages
            .iter()
            .map(|message| VisibleMessage {
                role: &message.role,
                content: &message.content,
                interrupted: message.interrupted,
            })
            .collect::<Vec<_>>();
        let suggestions = slash::suggestions(&self.input);
        let selected_suggestion = if suggestions.is_empty() {
            0
        } else {
            self.completion_index % suggestions.len()
        };
        terminal.draw(|frame| {
            chat::render(
                frame,
                ChatView {
                    session_name: &self.session.title,
                    provider: self.agent.provider_id(),
                    model: self.agent.model(),
                    generating: self.stream.is_some(),
                    input_tokens: self.usage.input_tokens,
                    output_tokens: self.usage.output_tokens,
                    error: self.error.as_deref(),
                    messages: &messages,
                    streaming_text: &self.streaming_text,
                    reasoning_text: &self.reasoning_text,
                    input: &self.input,
                    slash_suggestions: &suggestions,
                    selected_suggestion,
                },
            );
        })?;
        Ok(())
    }

    async fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return match key.code {
                KeyCode::Char('c') => {
                    if self.stream.is_some() {
                        self.cancel_generation()?;
                    }
                    Ok(false)
                }
                KeyCode::Char('n') => {
                    if self.stream.is_some() {
                        self.cancel_generation()?;
                    }
                    self.open_session(self.store.create_session()?)?;
                    Ok(true)
                }
                KeyCode::Char('l') => {
                    if self.stream.is_some() {
                        self.cancel_generation()?;
                    }
                    self.switch_session()?;
                    Ok(true)
                }
                KeyCode::Char('p') => {
                    if self.stream.is_none() {
                        self.input = "/provider ".into();
                        self.completion_index = 0;
                    }
                    Ok(true)
                }
                KeyCode::Char('o') => {
                    if self.stream.is_none() {
                        self.handle_slash_command("/help").await;
                    }
                    Ok(true)
                }
                _ => Ok(true),
            };
        }

        if self.stream.is_none() && self.handle_completion_key(key.code) {
            return Ok(true);
        }

        match key.code {
            KeyCode::Esc if self.stream.is_some() => self.cancel_generation()?,
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::SHIFT) => {
                if self.stream.is_none() {
                    self.input.push('\n');
                }
            }
            KeyCode::Enter => self.send().await?,
            KeyCode::Backspace if self.stream.is_none() => {
                self.input.pop();
                self.completion_index = 0;
            }
            KeyCode::Char(character) if self.stream.is_none() => {
                self.input.push(character);
                self.completion_index = 0;
            }
            _ => {}
        }
        Ok(true)
    }

    fn handle_completion_key(&mut self, key: KeyCode) -> bool {
        let suggestions = slash::suggestions(&self.input);
        if suggestions.is_empty() {
            return false;
        }
        match key {
            KeyCode::Up => {
                self.completion_index = self
                    .completion_index
                    .checked_sub(1)
                    .unwrap_or(suggestions.len() - 1);
                true
            }
            KeyCode::Down => {
                self.completion_index = (self.completion_index + 1) % suggestions.len();
                true
            }
            KeyCode::Tab => {
                let command = suggestions[self.completion_index % suggestions.len()];
                self.input.clear();
                self.input.push_str(command.name);
                if command.takes_argument {
                    self.input.push(' ');
                }
                self.completion_index = 0;
                true
            }
            _ => false,
        }
    }

    async fn send(&mut self) -> Result<()> {
        if self.stream.is_some() || self.input.trim().is_empty() {
            return Ok(());
        }
        self.error = None;
        let input = std::mem::take(&mut self.input);
        if input.trim_start().starts_with('/') {
            self.handle_slash_command(input.trim()).await;
            return Ok(());
        }
        self.store
            .save_message(&self.session.id, Role::User, &input, false)?;
        self.session.title = self
            .store
            .name_session_from_message(&self.session.id, &input)?;
        self.messages.push(StoredMessage {
            role: Role::User,
            content: input,
            interrupted: false,
        });
        let history = self.store.history(&self.session.id)?;
        let cancellation = Arc::new(AtomicBool::new(false));
        self.stream = Some(self.agent.reply(&history, cancellation.clone()).await?);
        self.cancellation = Some(cancellation);
        self.streaming_text.clear();
        self.reasoning_text.clear();
        Ok(())
    }

    async fn handle_slash_command(&mut self, command: &str) {
        self.error = None;
        if let Err(error) = self.execute_slash_command(command).await {
            self.error = Some(error.to_string());
        }
    }

    async fn execute_slash_command(&mut self, command: &str) -> Result<()> {
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
            "/models" => {
                let mut models = self.agent.models().await?;
                models.sort_by(|left, right| left.id.cmp(&right.id));
                let list = models
                    .into_iter()
                    .map(|model| model.id)
                    .collect::<Vec<_>>()
                    .join("\n");
                self.add_system_message(format!("可用模型：\n{list}"));
            }
            "/model" => {
                if let Some(model) = arguments.next() {
                    anyhow::ensure!(arguments.next().is_none(), "用法：/model <模型 ID>");
                    self.switch_model(model).await?;
                } else {
                    self.add_system_message(format!("当前模型：{}", self.agent.model()));
                }
            }
            "/new" => {
                anyhow::ensure!(arguments.next().is_none(), "用法：/new");
                self.open_session(self.store.create_session()?)?;
                self.add_system_message("已新建会话。");
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

    fn add_system_message(&mut self, content: impl Into<String>) {
        self.messages.push(StoredMessage {
            role: Role::System,
            content: content.into(),
            interrupted: false,
        });
    }

    fn handle_stream_event(&mut self, event: StreamEvent) -> Result<()> {
        match event {
            StreamEvent::TextDelta(delta) => self.streaming_text.push_str(&delta),
            StreamEvent::ReasoningDelta(delta) if self.show_reasoning => {
                self.reasoning_text.push_str(&delta);
            }
            StreamEvent::ReasoningDelta(_) => {}
            StreamEvent::Usage(usage) => self.usage = usage,
            StreamEvent::Completed => self.finish_generation(false)?,
            StreamEvent::Failed(error) => {
                self.error = Some(if error == "cancelled" {
                    "生成已取消".into()
                } else {
                    error
                });
                self.finish_generation(true)?;
            }
        }
        Ok(())
    }

    fn cancel_generation(&mut self) -> Result<()> {
        if let Some(cancellation) = &self.cancellation {
            cancellation.store(true, Ordering::Relaxed);
        }
        self.error = Some("生成已取消".into());
        self.finish_generation(true)
    }

    fn finish_generation(&mut self, interrupted: bool) -> Result<()> {
        self.stream = None;
        self.cancellation = None;
        self.reasoning_text.clear();
        if !self.streaming_text.is_empty() {
            let content = std::mem::take(&mut self.streaming_text);
            self.store
                .save_message(&self.session.id, Role::Assistant, &content, interrupted)?;
            self.messages.push(StoredMessage {
                role: Role::Assistant,
                content,
                interrupted,
            });
        }
        Ok(())
    }

    fn switch_session(&mut self) -> Result<()> {
        let sessions = self.store.list_sessions()?;
        if sessions.len() < 2 {
            self.error = Some("还没有其他会话；Ctrl+N 可以新建".into());
            return Ok(());
        }
        let current = sessions
            .iter()
            .position(|session| session.id == self.session.id)
            .unwrap_or(0);
        let next = sessions[(current + 1) % sessions.len()].clone();
        self.open_session(next)
    }

    fn open_session(&mut self, session: SessionSummary) -> Result<()> {
        self.messages = self.store.load_messages(&session.id)?;
        self.session = session;
        self.input.clear();
        self.streaming_text.clear();
        self.reasoning_text.clear();
        self.error = None;
        Ok(())
    }
}

enum LoopEvent {
    Terminal(Option<Result<Event, io::Error>>),
    Stream(Option<StreamEvent>),
}

async fn next_stream_event(stream: &mut Option<ChatStream>) -> Option<StreamEvent> {
    match stream {
        Some(stream) => stream.next().await,
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::mock::MockProvider;

    #[tokio::test]
    async fn slash_completion_and_model_selection_are_persistent() {
        let directory = tempfile::tempdir().unwrap();
        let config_path = directory.path().join("config.toml");
        let mut config = AppConfig::default();
        config.chat.provider = "mock".into();
        config.chat.model = "komari-mock".into();
        let store = Store::open(directory.path().join("chat.sqlite3")).unwrap();
        let agent = ChatAgent::new(Arc::new(MockProvider::immediate()), "komari-mock");
        let mut app = ChatApp::new(agent, store, config, config_path.clone(), None).unwrap();

        app.input = "/pro".into();
        assert!(app.handle_completion_key(KeyCode::Down));
        assert!(app.handle_completion_key(KeyCode::Tab));
        assert_eq!(app.input, "/provider ");
        app.input.clear();

        app.execute_slash_command("/model komari-mock")
            .await
            .unwrap();

        let saved = AppConfig::load(&config_path).unwrap();
        assert_eq!(saved.chat.provider, "mock");
        assert_eq!(saved.chat.model, "komari-mock");
        assert_eq!(app.messages.last().unwrap().role, Role::System);
        assert!(app.store.history(&app.session.id).unwrap().is_empty());
    }
}
