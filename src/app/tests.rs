use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{ChatApp, ChatFocus};
use crate::{
    agent::ChatAgent,
    config::AppConfig,
    memory::Store,
    provider::{Role, mock::MockProvider},
};

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

    app.open_model_picker().await.unwrap();
    assert_eq!(app.model_picker.as_ref().unwrap().models, ["komari-mock"]);
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
    assert_eq!(saved.chat.provider, "mock");
    assert_eq!(saved.chat.model, "komari-mock");
    assert_eq!(app.messages.last().unwrap().role, Role::System);
    assert!(app.store.history(&app.session.id).unwrap().is_empty());
}

#[tokio::test]
async fn t_focuses_history_and_j_k_scroll_without_stealing_text_input() {
    let directory = tempfile::tempdir().unwrap();
    let config_path = directory.path().join("config.toml");
    let mut config = AppConfig::default();
    config.chat.provider = "mock".into();
    config.chat.model = "komari-mock".into();
    let store = Store::open(directory.path().join("chat.sqlite3")).unwrap();
    let agent = ChatAgent::new(Arc::new(MockProvider::immediate()), "komari-mock");
    let mut app = ChatApp::new(agent, store, config, config_path, None).unwrap();

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
    config.chat.provider = "mock".into();
    config.chat.model = "komari-mock".into();
    let store = Store::open(directory.path().join("chat.sqlite3")).unwrap();
    let agent = ChatAgent::new(Arc::new(MockProvider::immediate()), "komari-mock");
    let mut app = ChatApp::new(agent, store, config, config_path, None).unwrap();
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
