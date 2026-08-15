use std::io::{self, stdout};
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use anyhow::Result;
use crossterm::{
    event::{Event, EventStream, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures_util::StreamExt;
use ratatui::{Terminal, backend::CrosstermBackend};
use tokio::time::{Duration, MissedTickBehavior};

use crate::{
    agent::{ChatAgent, ReplyFuture},
    config::AppConfig,
    memory::{SessionSummary, Store, StoredMessage},
    provider::{ChatStream, Role, StreamEvent, TokenUsage},
    tui::{
        chat::{self, ChatView, VisibleMessage},
        slash,
    },
};

mod commands;
mod credential_input;
mod input;

use credential_input::InputMode;

pub async fn run(
    agent: Option<ChatAgent>,
    store: Store,
    config: AppConfig,
    config_path: PathBuf,
    process_api_key: Option<String>,
) -> Result<()> {
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

struct ModelPicker {
    models: Vec<String>,
    selected: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum ChatFocus {
    #[default]
    Input,
    History,
}

struct ChatApp {
    agent: Option<ChatAgent>,
    store: Store,
    config: AppConfig,
    config_path: PathBuf,
    process_api_key: Option<String>,
    session: SessionSummary,
    messages: Vec<StoredMessage>,
    input: String,
    completion_index: usize,
    input_mode: InputMode,
    model_picker: Option<ModelPicker>,
    focus: ChatFocus,
    history_scroll: u16,
    stream_start: Option<ReplyFuture>,
    stream: Option<ChatStream>,
    cancellation: Option<Arc<AtomicBool>>,
    pending_send: bool,
    streaming_text: String,
    reasoning_text: String,
    show_reasoning: bool,
    usage: TokenUsage,
    error: Option<String>,
}

impl ChatApp {
    fn new(
        agent: Option<ChatAgent>,
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
            input_mode: InputMode::Chat,
            model_picker: None,
            focus: ChatFocus::Input,
            history_scroll: 0,
            stream_start: None,
            stream: None,
            cancellation: None,
            pending_send: false,
            streaming_text: String::new(),
            reasoning_text: String::new(),
            usage: TokenUsage::default(),
            error: None,
        })
    }

    fn provider_id(&self) -> &str {
        self.agent
            .as_ref()
            .map_or(&self.config.chat.provider, |agent| agent.provider_id())
    }

    fn model(&self) -> &str {
        self.agent
            .as_ref()
            .map_or(&self.config.chat.model, ChatAgent::model)
    }

    fn active_agent(&self) -> Result<&ChatAgent> {
        self.agent.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "尚未配置 {} API Key；输入 /login 后直接在当前输入框中登录",
                self.config.chat.provider
            )
        })
    }

    fn is_generating(&self) -> bool {
        self.stream_start.is_some() || self.stream.is_some()
    }

    async fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
        let mut events = EventStream::new();
        let mut redraw = tokio::time::interval(Duration::from_millis(33));
        redraw.set_missed_tick_behavior(MissedTickBehavior::Skip);
        // Consume the interval's immediate first tick; the initial frame is drawn explicitly.
        redraw.tick().await;
        self.draw(terminal)?;

        loop {
            let next = if self.stream_start.is_some() {
                tokio::select! {
                    terminal_event = events.next() => LoopEvent::Terminal(terminal_event),
                    result = next_stream_start(&mut self.stream_start) => LoopEvent::StreamStarted(result),
                    _ = redraw.tick() => LoopEvent::Redraw,
                }
            } else if self.stream.is_some() {
                tokio::select! {
                    terminal_event = events.next() => LoopEvent::Terminal(terminal_event),
                    stream_event = next_stream_event(&mut self.stream) => LoopEvent::Stream(stream_event),
                    _ = redraw.tick() => LoopEvent::Redraw,
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
                    self.draw(terminal)?;
                }
                LoopEvent::Terminal(Some(Ok(_))) => self.draw(terminal)?,
                LoopEvent::Terminal(Some(Err(error))) => return Err(error.into()),
                LoopEvent::Terminal(None) => break,
                LoopEvent::StreamStarted(result) => {
                    self.handle_stream_started(result)?;
                    self.draw(terminal)?;
                }
                LoopEvent::Stream(Some(event)) => {
                    self.handle_stream_event(event)?;
                    if !self.is_generating() {
                        if self.pending_send && self.error.is_none() {
                            self.pending_send = false;
                            self.send()?;
                        }
                        self.draw(terminal)?;
                    }
                }
                LoopEvent::Stream(None) => {
                    self.error = Some("Provider 在完成前关闭了连接".into());
                    self.pending_send = false;
                    self.finish_generation(true)?;
                    self.draw(terminal)?;
                }
                LoopEvent::Redraw => self.draw(terminal)?,
            }
        }
        if self.is_generating() {
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
        let displayed_input = self.input_mode.display(&self.input);
        let suggestions = if self.input_mode.is_credential() {
            Vec::new()
        } else {
            slash::suggestions(&self.input)
        };
        let selected_suggestion = self
            .completion_index
            .checked_rem(suggestions.len())
            .unwrap_or(0);
        terminal.draw(|frame| {
            chat::render(
                frame,
                ChatView {
                    session_name: &self.session.title,
                    provider: self.provider_id(),
                    model: self.model(),
                    generating: self.is_generating(),
                    input_tokens: self.usage.input_tokens,
                    output_tokens: self.usage.output_tokens,
                    error: self.error.as_deref(),
                    messages: &messages,
                    streaming_text: &self.streaming_text,
                    reasoning_text: &self.reasoning_text,
                    input: &displayed_input,
                    input_title: self.input_mode.title(),
                    credential_entry: self.input_mode.is_credential(),
                    message_queued: self.pending_send,
                    bold_text: self.config.display.bold_text,
                    border_style: self.config.display.border_style,
                    slash_suggestions: &suggestions,
                    selected_suggestion,
                    history_focused: self.focus == ChatFocus::History,
                    history_scroll: self.history_scroll,
                    model_picker: self
                        .model_picker
                        .as_ref()
                        .map(|picker| chat::SelectionView {
                            title: "选择模型 · Enter 确认 · Esc 关闭",
                            options: &picker.models,
                            selected: picker.selected,
                        }),
                },
            );
        })?;
        Ok(())
    }

    fn send(&mut self) -> Result<()> {
        if self.is_generating() || self.input.trim().is_empty() {
            return Ok(());
        }
        self.pending_send = false;
        self.error = None;
        self.focus = ChatFocus::Input;
        self.history_scroll = 0;
        self.active_agent()?;
        let input = std::mem::take(&mut self.input);
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
        self.streaming_text.clear();
        self.reasoning_text.clear();
        self.stream_start = Some(
            self.active_agent()?
                .start_reply(history, cancellation.clone()),
        );
        self.cancellation = Some(cancellation);
        Ok(())
    }

    fn handle_stream_started(&mut self, result: Result<ChatStream>) -> Result<()> {
        self.stream_start = None;
        match result {
            Ok(stream) => self.stream = Some(stream),
            Err(error) => {
                self.error = Some(error.to_string());
                self.pending_send = false;
                self.finish_generation(true)?;
            }
        }
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
        self.stream_start = None;
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
        self.model_picker = None;
        self.focus = ChatFocus::Input;
        self.history_scroll = 0;
        self.error = None;
        Ok(())
    }
}

enum LoopEvent {
    Terminal(Option<Result<Event, io::Error>>),
    StreamStarted(Result<ChatStream>),
    Stream(Option<StreamEvent>),
    Redraw,
}

async fn next_stream_start(start: &mut Option<ReplyFuture>) -> Result<ChatStream> {
    start
        .as_mut()
        .expect("stream start future is present while selected")
        .await
}

async fn next_stream_event(stream: &mut Option<ChatStream>) -> Option<StreamEvent> {
    match stream {
        Some(stream) => stream.next().await,
        None => None,
    }
}

#[cfg(test)]
mod tests;
