use std::sync::Arc;

use async_trait::async_trait;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use futures_util::stream;

use super::{ChatApp, ChatFocus, InputMode};
use crate::{
    agent::ChatAgent,
    config::{AppConfig, BorderStyle},
    memory::Store,
    provider::{
        ChatProvider, ChatRequest, ChatStream, ModelInfo, ProviderCapabilities, Role, StreamEvent,
    },
};

#[derive(Clone, Debug)]
struct TestProvider;

#[async_trait]
impl ChatProvider for TestProvider {
    fn id(&self) -> &'static str {
        "test"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            streaming: true,
            reasoning: true,
        }
    }

    async fn list_models(&self) -> anyhow::Result<Vec<ModelInfo>> {
        Ok(vec![ModelInfo {
            id: "test-model".into(),
            display_name: "test-model".into(),
        }])
    }

    async fn stream_chat(&self, _request: ChatRequest) -> anyhow::Result<ChatStream> {
        Ok(Box::pin(stream::iter([StreamEvent::Completed])))
    }

    async fn health_check(&self) -> anyhow::Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn slash_completion_and_model_selection_are_persistent() {
    let directory = tempfile::tempdir().unwrap();
    let config_path = directory.path().join("config.toml");
    let mut config = AppConfig::default();
    config.chat.provider = "test".into();
    config.chat.model = "test-model".into();
    let store = Store::open(directory.path().join("chat.sqlite3")).unwrap();
    let agent = ChatAgent::new(Arc::new(TestProvider), "test-model");
    let mut app = ChatApp::new(Some(agent), store, config, config_path.clone(), None).unwrap();

    app.input = "/pro".into();
    assert!(app.handle_completion_key(KeyCode::Down));
    assert!(app.handle_completion_key(KeyCode::Tab));
    assert_eq!(app.input, "/provider ");
    app.input.clear();

    app.open_model_picker().await.unwrap();
    assert_eq!(app.model_picker.as_ref().unwrap().models, ["test-model"]);
    app.model_picker
        .as_mut()
        .unwrap()
        .models
        .push("another-model".into());
    app.handle_model_picker_key(KeyEvent::from(KeyCode::Down))
        .await
        .unwrap();
    assert_eq!(app.model_picker.as_ref().unwrap().selected, 1);
    app.handle_model_picker_key(KeyEvent::from(KeyCode::Up))
        .await
        .unwrap();
    assert_eq!(app.model_picker.as_ref().unwrap().selected, 0);
    app.handle_model_picker_key(KeyEvent::from(KeyCode::Enter))
        .await
        .unwrap();
    assert!(app.model_picker.is_none());

    let saved = AppConfig::load(&config_path).unwrap();
    assert_eq!(saved.chat.provider, "test");
    assert_eq!(saved.chat.model, "test-model");
    assert_eq!(app.messages.last().unwrap().role, Role::System);
    assert!(app.store.history(&app.session.id).unwrap().is_empty());
}

#[tokio::test]
async fn enter_executes_the_selected_slash_suggestion() {
    let directory = tempfile::tempdir().unwrap();
    let config_path = directory.path().join("config.toml");
    let mut config = AppConfig::default();
    config.chat.provider = "test".into();
    config.chat.model = "test-model".into();
    let store = Store::open(directory.path().join("chat.sqlite3")).unwrap();
    let agent = ChatAgent::new(Arc::new(TestProvider), "test-model");
    let mut app = ChatApp::new(Some(agent), store, config, config_path, None).unwrap();

    app.input = "/pro".into();
    app.handle_key(KeyEvent::from(KeyCode::Enter))
        .await
        .unwrap();

    assert!(app.error.is_none());
    assert!(app.input.is_empty());
    assert!(
        app.messages
            .last()
            .unwrap()
            .content
            .contains("可用 Provider")
    );
}

#[tokio::test]
async fn login_command_enters_masked_input_and_escape_cancels() {
    let directory = tempfile::tempdir().unwrap();
    let config_path = directory.path().join("config.toml");
    let mut config = AppConfig::default();
    config.chat.provider = "test".into();
    config.chat.model = "test-model".into();
    let store = Store::open(directory.path().join("chat.sqlite3")).unwrap();
    let agent = ChatAgent::new(Arc::new(TestProvider), "test-model");
    let mut app = ChatApp::new(Some(agent), store, config, config_path, None).unwrap();

    app.execute_slash_command("/login deepseek").await.unwrap();
    assert!(matches!(
        app.input_mode,
        InputMode::Credential { ref provider_id } if provider_id == "deepseek"
    ));
    for character in "secret".chars() {
        app.handle_key(KeyEvent::from(KeyCode::Char(character)))
            .await
            .unwrap();
    }
    assert_eq!(app.input_mode.display(&app.input), "••••••");
    assert!(!app.input_mode.display(&app.input).contains("secret"));

    app.handle_key(KeyEvent::from(KeyCode::Esc)).await.unwrap();
    assert_eq!(app.input_mode, InputMode::Chat);
    assert!(app.input.is_empty());
}

#[tokio::test]
async fn login_is_available_when_starting_without_a_credential() {
    let directory = tempfile::tempdir().unwrap();
    let config_path = directory.path().join("config.toml");
    let config = AppConfig::default();
    let store = Store::open(directory.path().join("chat.sqlite3")).unwrap();
    let mut app = ChatApp::new(None, store, config, config_path, None).unwrap();

    app.input = "/log".into();
    app.handle_key(KeyEvent::from(KeyCode::Enter))
        .await
        .unwrap();

    assert!(matches!(
        app.input_mode,
        InputMode::Credential { ref provider_id } if provider_id == "deepseek"
    ));
}
#[tokio::test]
async fn typing_and_enter_queue_a_message_while_streaming() {
    let directory = tempfile::tempdir().unwrap();
    let config_path = directory.path().join("config.toml");
    let mut config = AppConfig::default();
    config.chat.provider = "test".into();
    config.chat.model = "test-model".into();
    let store = Store::open(directory.path().join("chat.sqlite3")).unwrap();
    let agent = ChatAgent::new(Arc::new(TestProvider), "test-model");
    let mut app = ChatApp::new(Some(agent), store, config, config_path, None).unwrap();
    app.stream = Some(Box::pin(stream::pending::<StreamEvent>()));

    for character in "下一条".chars() {
        app.handle_key(KeyEvent::from(KeyCode::Char(character)))
            .await
            .unwrap();
    }
    app.handle_key(KeyEvent::from(KeyCode::Enter))
        .await
        .unwrap();

    assert_eq!(app.input, "下一条");
    assert!(app.pending_send);
    assert!(app.error.as_deref().unwrap().contains("已排队"));
}

#[tokio::test]
async fn appearance_commands_persist_border_and_text_weight() {
    let directory = tempfile::tempdir().unwrap();
    let config_path = directory.path().join("config.toml");
    let mut config = AppConfig::default();
    config.chat.provider = "test".into();
    config.chat.model = "test-model".into();
    let store = Store::open(directory.path().join("chat.sqlite3")).unwrap();
    let agent = ChatAgent::new(Arc::new(TestProvider), "test-model");
    let mut app = ChatApp::new(Some(agent), store, config, config_path.clone(), None).unwrap();

    app.execute_slash_command("/border double").await.unwrap();
    app.execute_slash_command("/text normal").await.unwrap();

    assert_eq!(app.config.display.border_style, BorderStyle::Double);
    assert!(!app.config.display.bold_text);
    let saved = AppConfig::load(&config_path).unwrap();
    assert_eq!(saved.display.border_style, BorderStyle::Double);
    assert!(!saved.display.bold_text);
}
#[tokio::test]
async fn t_focuses_history_and_j_k_scroll_without_stealing_text_input() {
    let directory = tempfile::tempdir().unwrap();
    let config_path = directory.path().join("config.toml");
    let mut config = AppConfig::default();
    config.chat.provider = "test".into();
    config.chat.model = "test-model".into();
    let store = Store::open(directory.path().join("chat.sqlite3")).unwrap();
    let agent = ChatAgent::new(Arc::new(TestProvider), "test-model");
    let mut app = ChatApp::new(Some(agent), store, config, config_path, None).unwrap();

    app.handle_key(KeyEvent::from(KeyCode::Char('t')))
        .await
        .unwrap();
    assert_eq!(app.focus, ChatFocus::History);
    app.handle_key(KeyEvent::from(KeyCode::Char('k')))
        .await
        .unwrap();
    app.handle_key(KeyEvent::from(KeyCode::Char('k')))
        .await
        .unwrap();
    assert_eq!(app.history_scroll, 2);

    app.handle_key(KeyEvent::from(KeyCode::Char('j')))
        .await
        .unwrap();
    assert_eq!(app.history_scroll, 1);
    app.handle_key(KeyEvent::from(KeyCode::Char('t')))
        .await
        .unwrap();
    assert_eq!(app.focus, ChatFocus::Input);
    assert_eq!(app.history_scroll, 0);

    app.input = "rus".into();
    app.handle_key(KeyEvent::from(KeyCode::Char('t')))
        .await
        .unwrap();
    assert_eq!(app.focus, ChatFocus::Input);
    assert_eq!(app.input, "rust");

    app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL))
        .await
        .unwrap();
    assert_eq!(app.focus, ChatFocus::History);
    assert_eq!(app.input, "rust");
}

#[tokio::test]
async fn clear_starts_with_no_prior_conversation_history() {
    let directory = tempfile::tempdir().unwrap();
    let config_path = directory.path().join("config.toml");
    let mut config = AppConfig::default();
    config.chat.provider = "test".into();
    config.chat.model = "test-model".into();
    let store = Store::open(directory.path().join("chat.sqlite3")).unwrap();
    let agent = ChatAgent::new(Arc::new(TestProvider), "test-model");
    let mut app = ChatApp::new(Some(agent), store, config, config_path, None).unwrap();
    let previous_session = app.session.id.clone();
    app.store
        .save_message(&previous_session, Role::User, "上一段对话", false)
        .unwrap();

    app.execute_slash_command("/clear").await.unwrap();

    assert_ne!(app.session.id, previous_session);
    assert!(app.messages.is_empty());
    assert!(app.store.history(&app.session.id).unwrap().is_empty());
    assert_eq!(app.store.history(&previous_session).unwrap().len(), 1);

    app.input = "只属于新对话".into();
    app.send().await.unwrap();
    let current_history = app.store.history(&app.session.id).unwrap();
    assert_eq!(current_history.len(), 1);
    assert_eq!(current_history[0].content, "只属于新对话");
}
