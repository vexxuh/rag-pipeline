use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct EmbedKey {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing)]
    pub key_hash: String,
    pub key_prefix: String,
    pub allowed_domains: Vec<String>,
    pub system_prompt: String,
    pub rate_limit: i32,
    pub widget_title: String,
    pub primary_color: String,
    pub greeting_message: String,
    pub provider: String,
    pub model: String,
    #[serde(skip_serializing)]
    pub api_key_encrypted: String,
    pub total_conversations: i64,
    pub total_messages: i64,
    pub custom_css: String,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct UpdateEmbedKeyRequest {
    pub name: Option<String>,
    pub allowed_domains: Option<Vec<String>>,
    pub system_prompt: Option<String>,
    pub rate_limit: Option<i32>,
    pub widget_title: Option<String>,
    pub primary_color: Option<String>,
    pub greeting_message: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub api_key: Option<String>,
    pub custom_css: Option<String>,
}

const SELECT_COLS: &str =
    "id, name, key_hash, key_prefix, allowed_domains, system_prompt, rate_limit,
     widget_title, primary_color, greeting_message, provider, model, api_key_encrypted,
     custom_css, total_conversations, total_messages, is_active,
     to_char(created_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') as created_at_fmt,
     to_char(updated_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') as updated_at_fmt";

fn map_row(row: &sqlx::postgres::PgRow) -> EmbedKey {
    EmbedKey {
        id: row.get("id"),
        name: row.get("name"),
        key_hash: row.get("key_hash"),
        key_prefix: row.get("key_prefix"),
        allowed_domains: row.get("allowed_domains"),
        system_prompt: row.get("system_prompt"),
        rate_limit: row.get("rate_limit"),
        widget_title: row.get("widget_title"),
        primary_color: row.get("primary_color"),
        greeting_message: row.get("greeting_message"),
        provider: row.get("provider"),
        model: row.get("model"),
        api_key_encrypted: row.get("api_key_encrypted"),
        custom_css: row.get("custom_css"),
        total_conversations: row.get("total_conversations"),
        total_messages: row.get("total_messages"),
        is_active: row.get("is_active"),
        created_at: row.get("created_at_fmt"),
        updated_at: row.get("updated_at_fmt"),
    }
}

#[derive(Clone)]
pub struct EmbedKeyRepository {
    pool: PgPool,
}

impl EmbedKeyRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        id: &str,
        name: &str,
        key_hash: &str,
        key_prefix: &str,
        allowed_domains: &[String],
        system_prompt: &str,
        rate_limit: i32,
        widget_title: &str,
        primary_color: &str,
        greeting_message: &str,
        provider: &str,
        model: &str,
        api_key_encrypted: &str,
        custom_css: &str,
    ) -> Result<EmbedKey> {
        let sql = format!(
            "INSERT INTO embed_keys (id, name, key_hash, key_prefix, allowed_domains, system_prompt, rate_limit,
                widget_title, primary_color, greeting_message, provider, model, api_key_encrypted, custom_css)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
             RETURNING {SELECT_COLS}"
        );
        let row = sqlx::query(&sql)
            .bind(id)
            .bind(name)
            .bind(key_hash)
            .bind(key_prefix)
            .bind(allowed_domains)
            .bind(system_prompt)
            .bind(rate_limit)
            .bind(widget_title)
            .bind(primary_color)
            .bind(greeting_message)
            .bind(provider)
            .bind(model)
            .bind(api_key_encrypted)
            .bind(custom_css)
            .fetch_one(&self.pool)
            .await
            .context("Failed to create embed key")?;

        Ok(map_row(&row))
    }

    pub async fn find_by_hash(&self, key_hash: &str) -> Result<Option<EmbedKey>> {
        let sql = format!("SELECT {SELECT_COLS} FROM embed_keys WHERE key_hash = $1");
        let row = sqlx::query(&sql)
            .bind(key_hash)
            .fetch_optional(&self.pool)
            .await
            .context("Failed to find embed key by hash")?;

        Ok(row.as_ref().map(map_row))
    }

    pub async fn find_by_id(&self, id: &str) -> Result<Option<EmbedKey>> {
        let sql = format!("SELECT {SELECT_COLS} FROM embed_keys WHERE id = $1");
        let row = sqlx::query(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .context("Failed to find embed key by id")?;

        Ok(row.as_ref().map(map_row))
    }

    pub async fn list_all(&self) -> Result<Vec<EmbedKey>> {
        let sql = format!("SELECT {SELECT_COLS} FROM embed_keys ORDER BY created_at DESC");
        let rows = sqlx::query(&sql)
            .fetch_all(&self.pool)
            .await
            .context("Failed to list embed keys")?;

        Ok(rows.iter().map(map_row).collect())
    }

    pub async fn update(&self, id: &str, req: &UpdateEmbedKeyRequest) -> Result<Option<EmbedKey>> {
        // Build SET clause with proper types — we use an enum to track bind types
        // since sqlx needs correct Rust types for PostgreSQL columns.
        enum BindVal {
            Text(String),
            Int(i32),
            TextArray(Vec<String>),
        }

        let mut sets = Vec::new();
        let mut param_idx = 2u32; // $1 is id
        let mut binds: Vec<BindVal> = Vec::new();

        if let Some(ref name) = req.name {
            sets.push(format!("name = ${param_idx}"));
            binds.push(BindVal::Text(name.clone()));
            param_idx += 1;
        }
        if let Some(ref system_prompt) = req.system_prompt {
            sets.push(format!("system_prompt = ${param_idx}"));
            binds.push(BindVal::Text(system_prompt.clone()));
            param_idx += 1;
        }
        if let Some(ref widget_title) = req.widget_title {
            sets.push(format!("widget_title = ${param_idx}"));
            binds.push(BindVal::Text(widget_title.clone()));
            param_idx += 1;
        }
        if let Some(ref primary_color) = req.primary_color {
            sets.push(format!("primary_color = ${param_idx}"));
            binds.push(BindVal::Text(primary_color.clone()));
            param_idx += 1;
        }
        if let Some(ref greeting_message) = req.greeting_message {
            sets.push(format!("greeting_message = ${param_idx}"));
            binds.push(BindVal::Text(greeting_message.clone()));
            param_idx += 1;
        }
        if let Some(ref provider) = req.provider {
            sets.push(format!("provider = ${param_idx}"));
            binds.push(BindVal::Text(provider.clone()));
            param_idx += 1;
        }
        if let Some(ref model) = req.model {
            sets.push(format!("model = ${param_idx}"));
            binds.push(BindVal::Text(model.clone()));
            param_idx += 1;
        }
        if let Some(ref api_key) = req.api_key {
            sets.push(format!("api_key_encrypted = ${param_idx}"));
            binds.push(BindVal::Text(api_key.clone()));
            param_idx += 1;
        }
        if let Some(ref custom_css) = req.custom_css {
            sets.push(format!("custom_css = ${param_idx}"));
            binds.push(BindVal::Text(custom_css.clone()));
            param_idx += 1;
        }
        if let Some(rate_limit) = req.rate_limit {
            sets.push(format!("rate_limit = ${param_idx}"));
            binds.push(BindVal::Int(rate_limit));
            param_idx += 1;
        }
        if let Some(ref domains) = req.allowed_domains {
            sets.push(format!("allowed_domains = ${param_idx}"));
            binds.push(BindVal::TextArray(domains.clone()));
            let _ = param_idx;
        }

        if sets.is_empty() {
            return self.find_by_id(id).await;
        }

        sets.push("updated_at = NOW()".to_string());
        let set_clause = sets.join(", ");

        let sql = format!(
            "UPDATE embed_keys SET {set_clause} WHERE id = $1 RETURNING {SELECT_COLS}"
        );

        let mut query = sqlx::query(&sql).bind(id);
        for bind in binds {
            match bind {
                BindVal::Text(v) => query = query.bind(v),
                BindVal::Int(v) => query = query.bind(v),
                BindVal::TextArray(v) => query = query.bind(v),
            }
        }

        let row = query
            .fetch_optional(&self.pool)
            .await
            .context("Failed to update embed key")?;

        Ok(row.as_ref().map(map_row))
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM embed_keys WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .context("Failed to delete embed key")?;
        Ok(())
    }

    pub async fn toggle(&self, id: &str) -> Result<Option<EmbedKey>> {
        let sql = format!(
            "UPDATE embed_keys SET is_active = NOT is_active, updated_at = NOW() WHERE id = $1
             RETURNING {SELECT_COLS}"
        );
        let row = sqlx::query(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .context("Failed to toggle embed key")?;

        Ok(row.as_ref().map(map_row))
    }

    pub async fn increment_stats(&self, id: &str, conversations: i64, messages: i64) -> Result<()> {
        sqlx::query(
            "UPDATE embed_keys SET
                total_conversations = total_conversations + $2,
                total_messages = total_messages + $3,
                updated_at = NOW()
             WHERE id = $1"
        )
        .bind(id)
        .bind(conversations)
        .bind(messages)
        .execute(&self.pool)
        .await
        .context("Failed to increment embed key stats")?;
        Ok(())
    }
}
