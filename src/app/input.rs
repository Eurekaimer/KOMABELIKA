use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{ChatApp, ChatFocus};
use crate::tui::slash;

impl ChatApp {
    pub(super) async fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        if self.input_mode.is_credential() {
            return self.handle_credential_key(key).await;
        }
        if self.model_picker.is_some() {
            return self.handle_model_picker_key(key).await;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return match key.code {
                KeyCode::Char('c') => {
                    if self.is_generating() {
                        self.cancel_generation()?;
                    }
                    Ok(false)
                }
                KeyCode::Char('n') => {
                    if self.is_generating() {
                        self.cancel_generation()?;
                    }
                    self.open_session(self.store.create_session()?)?;
                    Ok(true)
                }
                KeyCode::Char('l') => {
                    if self.is_generating() {
                        self.cancel_generation()?;
                    }
                    self.switch_session()?;
                    Ok(true)
                }
                KeyCode::Char('p') => {
                    if !self.is_generating() {
                        self.show_model_picker().await;
                    }
                    Ok(true)
                }
                KeyCode::Char('t') => {
                    self.toggle_focus();
                    Ok(true)
                }
                KeyCode::Char('o') => {
                    if !self.is_generating() {
                        self.handle_slash_command("/help").await;
                    }
                    Ok(true)
                }
                KeyCode::Char('m') => {
                    self.submit_input().await?;
                    Ok(true)
                }
                _ => Ok(true),
            };
        }

        if key.code == KeyCode::Esc && self.is_generating() {
            self.cancel_generation()?;
            return Ok(true);
        }

        if self.focus == ChatFocus::Input && self.input.is_empty() && key.code == KeyCode::Char('t')
        {
            self.toggle_focus();
            return Ok(true);
        }

        if self.focus == ChatFocus::History {
            match key.code {
                KeyCode::Char('k') | KeyCode::Up => {
                    self.history_scroll = self.history_scroll.saturating_add(1);
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    self.history_scroll = self.history_scroll.saturating_sub(1);
                }
                KeyCode::PageUp => {
                    self.history_scroll = self.history_scroll.saturating_add(10);
                }
                KeyCode::PageDown => {
                    self.history_scroll = self.history_scroll.saturating_sub(10);
                }
                KeyCode::Char('g') | KeyCode::Home => self.history_scroll = u16::MAX,
                KeyCode::Char('G') | KeyCode::End => self.history_scroll = 0,
                KeyCode::Char('t') | KeyCode::Esc => self.toggle_focus(),
                _ => {}
            }
            return Ok(true);
        }

        if !self.is_generating() && self.handle_completion_key(key.code) {
            return Ok(true);
        }

        match key.code {
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.input.push('\n');
            }
            KeyCode::Enter => self.submit_input().await?,
            KeyCode::Backspace => {
                self.input.pop();
                self.completion_index = 0;
            }
            KeyCode::Char(character) => {
                self.input.push(character);
                self.completion_index = 0;
            }
            _ => {}
        }
        Ok(true)
    }

    async fn submit_input(&mut self) -> Result<()> {
        if self.is_generating() {
            if !self.input.trim().is_empty() {
                self.pending_send = true;
                self.error = Some("消息已排队，将在当前回复完成后发送".into());
            }
            return Ok(());
        }

        // Enter executes the highlighted command directly; Tab remains available
        // when the user wants to keep editing a command with arguments.
        if !self.input.chars().any(char::is_whitespace) {
            let suggestions = slash::suggestions(&self.input);
            if let Some(command) = suggestions.get(
                self.completion_index
                    .checked_rem(suggestions.len())
                    .unwrap_or(0),
            ) {
                let command = command.name;
                self.input.clear();
                self.completion_index = 0;
                self.handle_slash_command(command).await;
                return Ok(());
            }
        }
        if let Err(error) = self.send() {
            self.error = Some(error.to_string());
        }
        Ok(())
    }

    fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            ChatFocus::Input => ChatFocus::History,
            ChatFocus::History => ChatFocus::Input,
        };
        self.history_scroll = 0;
    }

    pub(super) fn handle_completion_key(&mut self, key: KeyCode) -> bool {
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
}
