use anyhow::{Context, Result};
use serde::Serialize;
use sqlx::PgPool;

#[derive(Debug, Clone, Serialize)]
pub struct WidgetSession {
    pub id: String,
    pub embed_key_id: String,
    pub session_id: String,
    pub message_count: i32,
    pub created_at: String,
    pub last_message_at: String,
}

#[derive(Clone)]
pub struct WidgetSessionRepository {
    pool: PgPool,
}

impl WidgetSessionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn get_or_create(&self, embed_key_id: &str, session_id: &str) -> Result<WidgetSession> {
        let id = uuid::Uuid::new_v4().to_string();
        let row = sqlx::query_as::<_, (String, String, String, i32, String, String)>(
            "INSERT INTO widget_sessions (id, embed_key_id, session_id)
             VALUES ($1, $2, $3)
             ON CONFLICT (embed_key_id, session_id) DO UPDATE SET embed_key_id = widget_sessions.embed_key_id
             RETURNING id, embed_key_id, session_id, message_count,
                to_char(created_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"'),
                to_char(last_message_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')"
        )
        .bind(&id)
        .bind(embed_key_id)
        .bind(session_id)
        .fetch_one(&self.pool)
        .await
        .context("Failed to get or create widget session")?;

        Ok(WidgetSession {
            id: row.0,
            embed_key_id: row.1,
            session_id: row.2,
            message_count: row.3,
            created_at: row.4,
            last_message_at: row.5,
        })
    }

    pub async fn increment_message_count(&self, embed_key_id: &str, session_id: &str) -> Result<i32> {
        let row = sqlx::query_as::<_, (i32,)>(
            "UPDATE widget_sessions SET message_count = message_count + 1, last_message_at = NOW()
             WHERE embed_key_id = $1 AND session_id = $2
             RETURNING message_count"
        )
        .bind(embed_key_id)
        .bind(session_id)
        .fetch_one(&self.pool)
        .await
        .context("Failed to increment widget session message count")?;

        Ok(row.0)
    }

    pub async fn get_message_count(&self, embed_key_id: &str, session_id: &str) -> Result<i32> {
        let row = sqlx::query_as::<_, (i32,)>(
            "SELECT COALESCE(
                (SELECT message_count FROM widget_sessions WHERE embed_key_id = $1 AND session_id = $2),
                0
            )"
        )
        .bind(embed_key_id)
        .bind(session_id)
        .fetch_one(&self.pool)
        .await
        .context("Failed to get widget session message count")?;

        Ok(row.0)
    }
}
