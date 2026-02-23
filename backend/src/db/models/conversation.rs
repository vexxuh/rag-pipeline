use anyhow::{Context, Result};
use serde::Serialize;
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Conversation {
    pub id: String,
    pub user_id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ConversationWithUser {
    pub id: String,
    pub user_id: String,
    pub username: String,
    pub email: String,
    pub title: String,
    pub message_count: i64,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct WidgetConversationLog {
    pub id: String,
    pub embed_key_id: String,
    pub embed_key_name: String,
    pub session_id: String,
    pub title: String,
    pub message_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Message {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Clone)]
pub struct ConversationRepository {
    pool: PgPool,
}

impl ConversationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, user_id: &str, title: &str) -> Result<Conversation> {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now();

        sqlx::query(
            "INSERT INTO conversations (id, user_id, title, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(&id)
        .bind(user_id)
        .bind(title)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .context("Failed to create conversation")?;

        Ok(Conversation {
            id,
            user_id: user_id.to_string(),
            title: title.to_string(),
            created_at: now.to_rfc3339(),
            updated_at: now.to_rfc3339(),
            deleted_at: None,
        })
    }

    pub async fn list_by_user(&self, user_id: &str) -> Result<Vec<Conversation>> {
        let rows = sqlx::query(
            "SELECT id, user_id, title,
                    to_char(created_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at,
                    to_char(updated_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS updated_at
             FROM conversations WHERE user_id = $1 AND deleted_at IS NULL ORDER BY updated_at DESC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to list conversations")?;

        let conversations = rows
            .iter()
            .map(|row| Conversation {
                id: row.get("id"),
                user_id: row.get("user_id"),
                title: row.get("title"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
                deleted_at: None,
            })
            .collect();

        Ok(conversations)
    }

    pub async fn get(&self, id: &str, user_id: &str) -> Result<Option<Conversation>> {
        let row = sqlx::query(
            "SELECT id, user_id, title,
                    to_char(created_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at,
                    to_char(updated_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS updated_at
             FROM conversations WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL",
        )
        .bind(id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to query conversation")?;

        Ok(row.map(|r| Conversation {
            id: r.get("id"),
            user_id: r.get("user_id"),
            title: r.get("title"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
            deleted_at: None,
        }))
    }

    pub async fn soft_delete(&self, id: &str, user_id: &str) -> Result<()> {
        let now = chrono::Utc::now();
        sqlx::query("UPDATE conversations SET deleted_at = $1 WHERE id = $2 AND user_id = $3 AND deleted_at IS NULL")
            .bind(now)
            .bind(id)
            .bind(user_id)
            .execute(&self.pool)
            .await
            .context("Failed to soft-delete conversation")?;

        Ok(())
    }

    pub async fn hard_delete_expired(&self) -> Result<i64> {
        let result = sqlx::query(
            "DELETE FROM conversations WHERE deleted_at IS NOT NULL AND deleted_at < NOW() - INTERVAL '30 days'",
        )
        .execute(&self.pool)
        .await
        .context("Failed to hard-delete expired conversations")?;

        Ok(result.rows_affected() as i64)
    }

    pub async fn update_title(&self, id: &str, title: &str) -> Result<()> {
        let now = chrono::Utc::now();
        sqlx::query("UPDATE conversations SET title = $1, updated_at = $2 WHERE id = $3")
            .bind(title)
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await
            .context("Failed to update title")?;

        Ok(())
    }

    pub async fn touch(&self, id: &str) -> Result<()> {
        let now = chrono::Utc::now();
        sqlx::query("UPDATE conversations SET updated_at = $1 WHERE id = $2")
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await
            .context("Failed to touch conversation")?;

        Ok(())
    }

    pub async fn add_message(
        &self,
        conversation_id: &str,
        role: &str,
        content: &str,
    ) -> Result<Message> {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now();

        sqlx::query(
            "INSERT INTO messages (id, conversation_id, role, content, created_at)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(&id)
        .bind(conversation_id)
        .bind(role)
        .bind(content)
        .bind(now)
        .execute(&self.pool)
        .await
        .context("Failed to add message")?;

        Ok(Message {
            id,
            conversation_id: conversation_id.to_string(),
            role: role.to_string(),
            content: content.to_string(),
            created_at: now.to_rfc3339(),
        })
    }

    pub async fn get_messages(&self, conversation_id: &str) -> Result<Vec<Message>> {
        let rows = sqlx::query(
            "SELECT id, conversation_id, role, content,
                    to_char(created_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at
             FROM messages WHERE conversation_id = $1 ORDER BY created_at ASC",
        )
        .bind(conversation_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get messages")?;

        let messages = rows
            .iter()
            .map(|row| Message {
                id: row.get("id"),
                conversation_id: row.get("conversation_id"),
                role: row.get("role"),
                content: row.get("content"),
                created_at: row.get("created_at"),
            })
            .collect();

        Ok(messages)
    }

    // ── Admin log queries (unscoped) ─────────────────────────

    pub async fn list_all(
        &self,
        user_id_filter: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ConversationWithUser>> {
        let (query, bind_user_id);
        if let Some(uid) = user_id_filter {
            bind_user_id = Some(uid.to_string());
            query = "SELECT c.id, c.user_id, u.username, u.email, c.title,
                            (SELECT COUNT(*) FROM messages m WHERE m.conversation_id = c.id) AS message_count,
                            to_char(c.created_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at,
                            to_char(c.updated_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS updated_at,
                            to_char(c.deleted_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS deleted_at
                     FROM conversations c
                     JOIN users u ON c.user_id = u.id
                     WHERE c.user_id = $1 AND (c.source IS NULL OR c.source != 'widget')
                     ORDER BY c.updated_at DESC
                     LIMIT $2 OFFSET $3";
        } else {
            bind_user_id = None;
            query = "SELECT c.id, c.user_id, u.username, u.email, c.title,
                            (SELECT COUNT(*) FROM messages m WHERE m.conversation_id = c.id) AS message_count,
                            to_char(c.created_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at,
                            to_char(c.updated_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS updated_at,
                            to_char(c.deleted_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS deleted_at
                     FROM conversations c
                     JOIN users u ON c.user_id = u.id
                     WHERE (c.source IS NULL OR c.source != 'widget')
                     ORDER BY c.updated_at DESC
                     LIMIT $1 OFFSET $2";
        }

        let rows = if let Some(ref uid) = bind_user_id {
            sqlx::query(query)
                .bind(uid)
                .bind(limit)
                .bind(offset)
                .fetch_all(&self.pool)
                .await
        } else {
            sqlx::query(query)
                .bind(limit)
                .bind(offset)
                .fetch_all(&self.pool)
                .await
        }
        .context("Failed to list all conversations")?;

        let conversations = rows
            .iter()
            .map(|row| ConversationWithUser {
                id: row.get("id"),
                user_id: row.get("user_id"),
                username: row.get("username"),
                email: row.get("email"),
                title: row.get("title"),
                message_count: row.get("message_count"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
                deleted_at: row.get("deleted_at"),
            })
            .collect();

        Ok(conversations)
    }

    pub async fn count_all(&self, user_id_filter: Option<&str>) -> Result<i64> {
        let count = if let Some(uid) = user_id_filter {
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM conversations WHERE user_id = $1 AND (source IS NULL OR source != 'widget')",
            )
            .bind(uid)
            .fetch_one(&self.pool)
            .await
        } else {
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM conversations WHERE (source IS NULL OR source != 'widget')",
            )
            .fetch_one(&self.pool)
            .await
        }
        .context("Failed to count conversations")?;

        Ok(count)
    }

    // ── Widget log queries (admin) ───────────────────────────

    pub async fn list_widget_conversations(
        &self,
        embed_key_id_filter: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<WidgetConversationLog>> {
        let (query, bind_ek_id);
        if let Some(ek_id) = embed_key_id_filter {
            bind_ek_id = Some(ek_id.to_string());
            query = "SELECT c.id, c.embed_key_id,
                            COALESCE(ek.name, 'Unknown') AS embed_key_name,
                            COALESCE(c.session_id, '') AS session_id,
                            c.title,
                            (SELECT COUNT(*) FROM messages m WHERE m.conversation_id = c.id) AS message_count,
                            to_char(c.created_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at,
                            to_char(c.updated_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS updated_at
                     FROM conversations c
                     LEFT JOIN embed_keys ek ON c.embed_key_id = ek.id
                     WHERE c.source = 'widget' AND c.embed_key_id = $1
                     ORDER BY c.updated_at DESC
                     LIMIT $2 OFFSET $3";
        } else {
            bind_ek_id = None;
            query = "SELECT c.id, c.embed_key_id,
                            COALESCE(ek.name, 'Unknown') AS embed_key_name,
                            COALESCE(c.session_id, '') AS session_id,
                            c.title,
                            (SELECT COUNT(*) FROM messages m WHERE m.conversation_id = c.id) AS message_count,
                            to_char(c.created_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at,
                            to_char(c.updated_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS updated_at
                     FROM conversations c
                     LEFT JOIN embed_keys ek ON c.embed_key_id = ek.id
                     WHERE c.source = 'widget'
                     ORDER BY c.updated_at DESC
                     LIMIT $1 OFFSET $2";
        }

        let rows = if let Some(ref ek_id) = bind_ek_id {
            sqlx::query(query)
                .bind(ek_id)
                .bind(limit)
                .bind(offset)
                .fetch_all(&self.pool)
                .await
        } else {
            sqlx::query(query)
                .bind(limit)
                .bind(offset)
                .fetch_all(&self.pool)
                .await
        }
        .context("Failed to list widget conversations")?;

        let conversations = rows
            .iter()
            .map(|row| WidgetConversationLog {
                id: row.get("id"),
                embed_key_id: row.get::<Option<String>, _>("embed_key_id").unwrap_or_default(),
                embed_key_name: row.get("embed_key_name"),
                session_id: row.get("session_id"),
                title: row.get("title"),
                message_count: row.get("message_count"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect();

        Ok(conversations)
    }

    pub async fn count_widget_conversations(
        &self,
        embed_key_id_filter: Option<&str>,
    ) -> Result<i64> {
        let count = if let Some(ek_id) = embed_key_id_filter {
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM conversations WHERE source = 'widget' AND embed_key_id = $1",
            )
            .bind(ek_id)
            .fetch_one(&self.pool)
            .await
        } else {
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM conversations WHERE source = 'widget'",
            )
            .fetch_one(&self.pool)
            .await
        }
        .context("Failed to count widget conversations")?;

        Ok(count)
    }

    pub async fn get_by_id(&self, id: &str) -> Result<Option<Conversation>> {
        let row = sqlx::query(
            "SELECT id, user_id, title,
                    to_char(created_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at,
                    to_char(updated_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS updated_at,
                    to_char(deleted_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS deleted_at
             FROM conversations WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to query conversation by id")?;

        Ok(row.map(|r| Conversation {
            id: r.get("id"),
            user_id: r.get("user_id"),
            title: r.get("title"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
            deleted_at: r.get("deleted_at"),
        }))
    }

    // ── Widget conversation methods ──────────────────────────

    pub async fn create_widget(
        &self,
        embed_key_id: &str,
        session_id: &str,
        title: &str,
    ) -> Result<Conversation> {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now();

        sqlx::query(
            "INSERT INTO conversations (id, user_id, title, created_at, updated_at, source, embed_key_id, session_id)
             VALUES ($1, '__widget__', $2, $3, $4, 'widget', $5, $6)",
        )
        .bind(&id)
        .bind(title)
        .bind(now)
        .bind(now)
        .bind(embed_key_id)
        .bind(session_id)
        .execute(&self.pool)
        .await
        .context("Failed to create widget conversation")?;

        Ok(Conversation {
            id,
            user_id: "__widget__".to_string(),
            title: title.to_string(),
            created_at: now.to_rfc3339(),
            updated_at: now.to_rfc3339(),
            deleted_at: None,
        })
    }

    pub async fn get_widget(
        &self,
        id: &str,
        session_id: &str,
        embed_key_id: &str,
    ) -> Result<Option<Conversation>> {
        let row = sqlx::query(
            "SELECT id, user_id, title,
                    to_char(created_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at,
                    to_char(updated_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS updated_at
             FROM conversations
             WHERE id = $1 AND session_id = $2 AND embed_key_id = $3
               AND source = 'widget' AND deleted_at IS NULL",
        )
        .bind(id)
        .bind(session_id)
        .bind(embed_key_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get widget conversation")?;

        Ok(row.map(|r| Conversation {
            id: r.get("id"),
            user_id: r.get("user_id"),
            title: r.get("title"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
            deleted_at: None,
        }))
    }

    pub async fn list_by_session(
        &self,
        session_id: &str,
        embed_key_id: &str,
    ) -> Result<Vec<Conversation>> {
        let rows = sqlx::query(
            "SELECT id, user_id, title,
                    to_char(created_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at,
                    to_char(updated_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS updated_at
             FROM conversations
             WHERE session_id = $1 AND embed_key_id = $2
               AND source = 'widget' AND deleted_at IS NULL
             ORDER BY updated_at DESC",
        )
        .bind(session_id)
        .bind(embed_key_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to list widget conversations")?;

        Ok(rows
            .iter()
            .map(|r| Conversation {
                id: r.get("id"),
                user_id: r.get("user_id"),
                title: r.get("title"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
                deleted_at: None,
            })
            .collect())
    }
}
