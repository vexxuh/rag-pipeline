use anyhow::{Context, Result};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyEntry {
    pub id: String,
    pub provider: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmPreferences {
    pub preferred_provider: String,
    pub preferred_model: String,
    pub preferred_embedding_model: String,
    pub system_prompt: String,
}

#[derive(Clone)]
pub struct SettingsRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SettingsRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    // ── API Keys ──────────────────────────────────────────────
    pub fn set_api_key(&self, user_id: &str, provider: &str, api_key: &str) -> Result<ApiKeyEntry> {
        let conn = self.pool.get().context("Failed to get db connection")?;
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO user_api_keys (id, user_id, provider, api_key, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(user_id, provider) DO UPDATE SET api_key = ?4, id = ?1",
            params![id, user_id, provider, api_key, now],
        )
        .context("Failed to upsert API key")?;

        Ok(ApiKeyEntry {
            id,
            provider: provider.to_string(),
            created_at: now,
        })
    }

    pub fn get_api_key(&self, user_id: &str, provider: &str) -> Result<Option<String>> {
        let conn = self.pool.get().context("Failed to get db connection")?;
        let mut stmt = conn.prepare(
            "SELECT api_key FROM user_api_keys WHERE user_id = ?1 AND provider = ?2",
        )?;

        let key = stmt
            .query_row(params![user_id, provider], |row| row.get::<_, String>(0))
            .optional()
            .context("Failed to query API key")?;

        Ok(key)
    }

    pub fn list_api_keys(&self, user_id: &str) -> Result<Vec<ApiKeyEntry>> {
        let conn = self.pool.get().context("Failed to get db connection")?;
        let mut stmt = conn.prepare(
            "SELECT id, provider, created_at FROM user_api_keys WHERE user_id = ?1 ORDER BY provider",
        )?;

        let entries = stmt
            .query_map(params![user_id], |row| {
                Ok(ApiKeyEntry {
                    id: row.get(0)?,
                    provider: row.get(1)?,
                    created_at: row.get(2)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to collect API keys")?;

        Ok(entries)
    }

    pub fn delete_api_key(&self, user_id: &str, provider: &str) -> Result<()> {
        let conn = self.pool.get().context("Failed to get db connection")?;
        conn.execute(
            "DELETE FROM user_api_keys WHERE user_id = ?1 AND provider = ?2",
            params![user_id, provider],
        )
        .context("Failed to delete API key")?;
        Ok(())
    }

    // ── LLM Preferences ──────────────────────────────────────
    pub fn get_preferences(&self, user_id: &str) -> Result<Option<LlmPreferences>> {
        let conn = self.pool.get().context("Failed to get db connection")?;
        let mut stmt = conn.prepare(
            "SELECT preferred_provider, preferred_model, preferred_embedding_model, system_prompt
             FROM user_llm_preferences WHERE user_id = ?1",
        )?;

        let prefs = stmt
            .query_row(params![user_id], |row| {
                Ok(LlmPreferences {
                    preferred_provider: row.get(0)?,
                    preferred_model: row.get(1)?,
                    preferred_embedding_model: row.get(2)?,
                    system_prompt: row.get(3)?,
                })
            })
            .optional()
            .context("Failed to query preferences")?;

        Ok(prefs)
    }

    pub fn set_preferences(&self, user_id: &str, prefs: &LlmPreferences) -> Result<()> {
        let conn = self.pool.get().context("Failed to get db connection")?;
        conn.execute(
            "INSERT INTO user_llm_preferences (user_id, preferred_provider, preferred_model, preferred_embedding_model, system_prompt)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(user_id) DO UPDATE SET
                preferred_provider = ?2,
                preferred_model = ?3,
                preferred_embedding_model = ?4,
                system_prompt = ?5",
            params![
                user_id,
                prefs.preferred_provider,
                prefs.preferred_model,
                prefs.preferred_embedding_model,
                prefs.system_prompt,
            ],
        )
        .context("Failed to upsert preferences")?;
        Ok(())
    }
}

trait OptionalRow<T> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error>;
}

impl<T> OptionalRow<T> for std::result::Result<T, rusqlite::Error> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error> {
        match self {
            Ok(val) => Ok(Some(val)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}
