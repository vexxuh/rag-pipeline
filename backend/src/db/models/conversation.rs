use anyhow::{Context, Result};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct Conversation {
    pub id: String,
    pub user_id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Message {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Clone)]
pub struct ConversationRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl ConversationRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    pub fn create(&self, user_id: &str, title: &str) -> Result<Conversation> {
        let conn = self.pool.get().context("Failed to get db connection")?;
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO conversations (id, user_id, title, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, user_id, title, now, now],
        )
        .context("Failed to create conversation")?;

        Ok(Conversation {
            id,
            user_id: user_id.to_string(),
            title: title.to_string(),
            created_at: now.clone(),
            updated_at: now,
        })
    }

    pub fn list_by_user(&self, user_id: &str) -> Result<Vec<Conversation>> {
        let conn = self.pool.get().context("Failed to get db connection")?;
        let mut stmt = conn.prepare(
            "SELECT id, user_id, title, created_at, updated_at
             FROM conversations WHERE user_id = ?1 ORDER BY updated_at DESC",
        )?;

        let rows = stmt
            .query_map(params![user_id], |row| {
                Ok(Conversation {
                    id: row.get(0)?,
                    user_id: row.get(1)?,
                    title: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to collect conversations")?;

        Ok(rows)
    }

    pub fn get(&self, id: &str, user_id: &str) -> Result<Option<Conversation>> {
        let conn = self.pool.get().context("Failed to get db connection")?;
        let result = conn
            .query_row(
                "SELECT id, user_id, title, created_at, updated_at
                 FROM conversations WHERE id = ?1 AND user_id = ?2",
                params![id, user_id],
                |row| {
                    Ok(Conversation {
                        id: row.get(0)?,
                        user_id: row.get(1)?,
                        title: row.get(2)?,
                        created_at: row.get(3)?,
                        updated_at: row.get(4)?,
                    })
                },
            )
            .optional();

        match result {
            Ok(c) => Ok(c),
            Err(e) => Err(anyhow::anyhow!("Query error: {e}")),
        }
    }

    pub fn delete(&self, id: &str, user_id: &str) -> Result<()> {
        let conn = self.pool.get().context("Failed to get db connection")?;
        conn.execute(
            "DELETE FROM conversations WHERE id = ?1 AND user_id = ?2",
            params![id, user_id],
        )
        .context("Failed to delete conversation")?;
        Ok(())
    }

    pub fn update_title(&self, id: &str, title: &str) -> Result<()> {
        let conn = self.pool.get().context("Failed to get db connection")?;
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE conversations SET title = ?1, updated_at = ?2 WHERE id = ?3",
            params![title, now, id],
        )
        .context("Failed to update title")?;
        Ok(())
    }

    pub fn touch(&self, id: &str) -> Result<()> {
        let conn = self.pool.get().context("Failed to get db connection")?;
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        Ok(())
    }

    pub fn add_message(
        &self,
        conversation_id: &str,
        role: &str,
        content: &str,
    ) -> Result<Message> {
        let conn = self.pool.get().context("Failed to get db connection")?;
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO messages (id, conversation_id, role, content, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, conversation_id, role, content, now],
        )
        .context("Failed to add message")?;

        Ok(Message {
            id,
            conversation_id: conversation_id.to_string(),
            role: role.to_string(),
            content: content.to_string(),
            created_at: now,
        })
    }

    pub fn get_messages(&self, conversation_id: &str) -> Result<Vec<Message>> {
        let conn = self.pool.get().context("Failed to get db connection")?;
        let mut stmt = conn.prepare(
            "SELECT id, conversation_id, role, content, created_at
             FROM messages WHERE conversation_id = ?1 ORDER BY created_at ASC",
        )?;

        let rows = stmt
            .query_map(params![conversation_id], |row| {
                Ok(Message {
                    id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    role: row.get(2)?,
                    content: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to collect messages")?;

        Ok(rows)
    }
}

trait OptionalRow {
    fn optional(self) -> Result<Option<Conversation>, rusqlite::Error>;
}

impl OptionalRow for std::result::Result<Conversation, rusqlite::Error> {
    fn optional(self) -> Result<Option<Conversation>, rusqlite::Error> {
        match self {
            Ok(c) => Ok(Some(c)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}
