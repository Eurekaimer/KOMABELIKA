use std::path::Path;

use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use crate::provider::{ChatMessage, Role};

const MIGRATION_V1: &str = r#"
CREATE TABLE sessions (
    id          TEXT PRIMARY KEY,
    title       TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);
CREATE TABLE messages (
    id           TEXT PRIMARY KEY,
    session_id   TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    role         TEXT NOT NULL CHECK (role IN ('user', 'assistant')),
    content      TEXT NOT NULL,
    interrupted  INTEGER NOT NULL DEFAULT 0,
    created_at   TEXT NOT NULL
);
CREATE INDEX messages_session_created ON messages(session_id, created_at);
PRAGMA user_version = 1;
"#;

pub struct Store {
    connection: Connection,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionSummary {
    pub id: String,
    pub title: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredMessage {
    pub role: Role,
    pub content: String,
    pub interrupted: bool,
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let connection = Connection::open(path).context("failed to open conversation database")?;
        let mut store = Self { connection };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&mut self) -> Result<()> {
        self.connection.execute_batch("PRAGMA foreign_keys = ON;")?;
        let version = self
            .connection
            .query_row("PRAGMA user_version", [], |row| row.get::<_, u32>(0))?;
        if version == 0 {
            let transaction = self.connection.transaction()?;
            transaction.execute_batch(MIGRATION_V1)?;
            transaction.commit()?;
        }
        Ok(())
    }

    pub fn latest_or_create_session(&self) -> Result<SessionSummary> {
        self.latest_session()?
            .map_or_else(|| self.create_session(), Ok)
    }

    pub fn latest_session(&self) -> Result<Option<SessionSummary>> {
        self.connection
            .query_row(
                "SELECT id, title FROM sessions ORDER BY updated_at DESC LIMIT 1",
                [],
                |row| {
                    Ok(SessionSummary {
                        id: row.get(0)?,
                        title: row.get(1)?,
                    })
                },
            )
            .optional()
            .context("failed to load latest session")
    }

    pub fn create_session(&self) -> Result<SessionSummary> {
        let session = SessionSummary {
            id: Uuid::new_v4().to_string(),
            title: "新的对话".into(),
        };
        let now = Utc::now().to_rfc3339();
        self.connection.execute(
            "INSERT INTO sessions (id, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
            params![session.id, session.title, now],
        )?;
        Ok(session)
    }

    pub fn list_sessions(&self) -> Result<Vec<SessionSummary>> {
        let mut statement = self
            .connection
            .prepare("SELECT id, title FROM sessions ORDER BY updated_at DESC")?;
        let rows = statement.query_map([], |row| {
            Ok(SessionSummary {
                id: row.get(0)?,
                title: row.get(1)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to list sessions")
    }

    pub fn save_message(
        &self,
        session_id: &str,
        role: Role,
        content: &str,
        interrupted: bool,
    ) -> Result<()> {
        let role = match role {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::System => anyhow::bail!("system messages are not persisted"),
        };
        let now = Utc::now().to_rfc3339();
        let transaction = self.connection.unchecked_transaction()?;
        transaction.execute(
            "INSERT INTO messages (id, session_id, role, content, interrupted, created_at)\
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                Uuid::new_v4().to_string(),
                session_id,
                role,
                content,
                interrupted,
                now
            ],
        )?;
        transaction.execute(
            "UPDATE sessions SET updated_at = ?1 WHERE id = ?2",
            params![now, session_id],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn name_session_from_message(&self, session_id: &str, message: &str) -> Result<String> {
        let title = message.chars().take(24).collect::<String>();
        let title = if title.is_empty() {
            "新的对话"
        } else {
            &title
        };
        self.connection.execute(
            "UPDATE sessions SET title = ?1 WHERE id = ?2 AND title = '新的对话'",
            params![title, session_id],
        )?;
        Ok(self.connection.query_row(
            "SELECT title FROM sessions WHERE id = ?1",
            [session_id],
            |row| row.get(0),
        )?)
    }

    pub fn load_messages(&self, session_id: &str) -> Result<Vec<StoredMessage>> {
        let mut statement = self.connection.prepare(
            "SELECT role, content, interrupted FROM messages WHERE session_id = ?1 ORDER BY created_at, rowid",
        )?;
        let rows = statement.query_map([session_id], |row| {
            let role: String = row.get(0)?;
            Ok(StoredMessage {
                role: if role == "user" {
                    Role::User
                } else {
                    Role::Assistant
                },
                content: row.get(1)?,
                interrupted: row.get(2)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to load messages")
    }

    pub fn history(&self, session_id: &str) -> Result<Vec<ChatMessage>> {
        Ok(self
            .load_messages(session_id)?
            .into_iter()
            .filter(|message| !message.interrupted)
            .map(|message| ChatMessage {
                role: message.role,
                content: message.content,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory_store() -> Store {
        Store::open(":memory:").unwrap()
    }

    #[test]
    fn migration_creates_schema() {
        let store = memory_store();
        let version = store
            .connection
            .query_row("PRAGMA user_version", [], |row| row.get::<_, u32>(0))
            .unwrap();
        assert_eq!(version, 1);
    }

    #[test]
    fn session_and_messages_survive_round_trip() {
        let store = memory_store();
        let session = store.create_session().unwrap();
        store
            .save_message(&session.id, Role::User, "我喜欢终端", false)
            .unwrap();
        store
            .save_message(&session.id, Role::Assistant, "……我记住了。", false)
            .unwrap();

        let messages = store.load_messages(&session.id).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].content, "我喜欢终端");
        assert!(!messages[1].interrupted);
    }
}
