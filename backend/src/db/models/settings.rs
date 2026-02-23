use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ApiKeyEntry {
    pub id: String,
    pub provider: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct LlmPreferences {
    pub preferred_provider: String,
    pub preferred_model: String,
    pub preferred_embedding_model: String,
    pub system_prompt: String,
}

#[derive(Clone)]
pub struct SettingsRepository {
    pool: PgPool,
}

impl SettingsRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ── API Keys ──────────────────────────────────────────────
    pub async fn set_api_key(
        &self,
        user_id: &str,
        provider: &str,
        api_key: &str,
    ) -> Result<ApiKeyEntry> {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now();

        let row = sqlx::query(
            "INSERT INTO user_api_keys (id, user_id, provider, api_key, created_at)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT(user_id, provider) DO UPDATE SET api_key = $4, id = $1
             RETURNING id, provider, to_char(created_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at",
        )
        .bind(&id)
        .bind(user_id)
        .bind(provider)
        .bind(api_key)
        .bind(now)
        .fetch_one(&self.pool)
        .await
        .context("Failed to upsert API key")?;

        Ok(ApiKeyEntry {
            id: row.get("id"),
            provider: row.get("provider"),
            created_at: row.get("created_at"),
        })
    }

    pub async fn get_api_key(&self, user_id: &str, provider: &str) -> Result<Option<String>> {
        let row = sqlx::query(
            "SELECT api_key FROM user_api_keys WHERE user_id = $1 AND provider = $2",
        )
        .bind(user_id)
        .bind(provider)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to query API key")?;

        Ok(row.map(|r| r.get("api_key")))
    }

    /// Find any API key for a provider (prefers admin users, then any user).
    /// Used as a fallback when an embed key has no dedicated API key.
    pub async fn get_any_api_key_for_provider(&self, provider: &str) -> Result<Option<String>> {
        let row = sqlx::query(
            "SELECT k.api_key FROM user_api_keys k
             JOIN users u ON u.id = k.user_id
             WHERE k.provider = $1
             ORDER BY CASE u.role WHEN 'admin' THEN 0 ELSE 1 END
             LIMIT 1",
        )
        .bind(provider)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to query fallback API key")?;

        Ok(row.map(|r| r.get("api_key")))
    }

    pub async fn list_api_keys(&self, user_id: &str) -> Result<Vec<ApiKeyEntry>> {
        let rows = sqlx::query(
            "SELECT id, provider, to_char(created_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at
             FROM user_api_keys WHERE user_id = $1 ORDER BY provider",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to list API keys")?;

        let entries = rows
            .iter()
            .map(|row| ApiKeyEntry {
                id: row.get("id"),
                provider: row.get("provider"),
                created_at: row.get("created_at"),
            })
            .collect();

        Ok(entries)
    }

    pub async fn delete_api_key(&self, user_id: &str, provider: &str) -> Result<()> {
        sqlx::query("DELETE FROM user_api_keys WHERE user_id = $1 AND provider = $2")
            .bind(user_id)
            .bind(provider)
            .execute(&self.pool)
            .await
            .context("Failed to delete API key")?;

        Ok(())
    }

    // ── LLM Preferences ──────────────────────────────────────
    pub async fn get_preferences(&self, user_id: &str) -> Result<Option<LlmPreferences>> {
        let row = sqlx::query(
            "SELECT preferred_provider, preferred_model, preferred_embedding_model, system_prompt
             FROM user_llm_preferences WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to query preferences")?;

        Ok(row.map(|r| LlmPreferences {
            preferred_provider: r.get("preferred_provider"),
            preferred_model: r.get("preferred_model"),
            preferred_embedding_model: r.get("preferred_embedding_model"),
            system_prompt: r.get("system_prompt"),
        }))
    }

    pub async fn set_preferences(&self, user_id: &str, prefs: &LlmPreferences) -> Result<()> {
        sqlx::query(
            "INSERT INTO user_llm_preferences
                 (user_id, preferred_provider, preferred_model, preferred_embedding_model, system_prompt)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT(user_id) DO UPDATE SET
                 preferred_provider      = $2,
                 preferred_model         = $3,
                 preferred_embedding_model = $4,
                 system_prompt           = $5",
        )
        .bind(user_id)
        .bind(&prefs.preferred_provider)
        .bind(&prefs.preferred_model)
        .bind(&prefs.preferred_embedding_model)
        .bind(&prefs.system_prompt)
        .execute(&self.pool)
        .await
        .context("Failed to upsert preferences")?;

        Ok(())
    }
}
