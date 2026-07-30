use std::io::{self, stdout};
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
    memory::{SessionSummary, Store, StoredMessage},
    provider::{ChatStream, Role, StreamEvent, TokenUsage},
    tui::chat::{self, ChatView, VisibleMessage},
};

pub async fn run(agent: ChatAgent, store: Store, show_reasoning: bool) -> Result<()> {
    let models = agent.models().await?;
    if !agent.capabilities().streaming || models.iter().all(|model| model.id != agent.model()) {
        anyhow::bail!("当前 Provider 不支持所选流式模型");
    }

    enable_raw_mode()?;
    let mut output = stdout();
    execute!(output, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(output);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let result = ChatApp::new(agent, store, show_reasoning)?
        .run(&mut terminal)
        .await;
    let restore_result = restore_terminal(&mut terminal);
    result.and(restore_result)
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
    session: SessionSummary,
    messages: Vec<StoredMessage>,
    input: String,
    stream: Option<ChatStream>,
    cancellation: Option<Arc<AtomicBool>>,
    streaming_text: String,
    reasoning_text: String,
    show_reasoning: bool,
    usage: TokenUsage,
    error: Option<String>,
}

impl ChatApp {
    fn new(agent: ChatAgent, store: Store, show_reasoning: bool) -> Result<Self> {
        let session = store.latest_or_create_session()?;
        let messages = store.load_messages(&session.id)?;
        Ok(Self {
            agent,
            store,
            session,
            messages,
            input: String::new(),
            stream: None,
            cancellation: None,
            streaming_text: String::new(),
            reasoning_text: String::new(),
            show_reasoning,
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
                    self.error = Some("MVP 当前只有 Mock Provider".into());
                    Ok(true)
                }
                KeyCode::Char('o') => {
                    self.error = Some("配置 TUI 将在 Provider 接入阶段开放".into());
                    Ok(true)
                }
                _ => Ok(true),
            };
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
            }
            KeyCode::Char(character) if self.stream.is_none() => self.input.push(character),
            _ => {}
        }
        Ok(true)
    }

    async fn send(&mut self) -> Result<()> {
        if self.stream.is_some() || self.input.trim().is_empty() {
            return Ok(());
        }
        self.error = None;
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
        self.stream = Some(self.agent.reply(&history, cancellation.clone()).await?);
        self.cancellation = Some(cancellation);
        self.streaming_text.clear();
        self.reasoning_text.clear();
        Ok(())
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
