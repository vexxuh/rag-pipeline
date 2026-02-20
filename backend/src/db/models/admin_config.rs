use anyhow::{Context, Result};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::services::llm_provider;

#[derive(Debug, Clone, Serialize)]
pub struct AdminProvider {
    pub id: String,
    pub provider_id: String,
    pub display_name: String,
    pub enabled: bool,
    pub supports_completion: bool,
    pub supports_embeddings: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdminModel {
    pub id: String,
    pub provider_id: String,
    pub model_id: String,
    pub display_name: String,
    pub model_type: String,
    pub is_default: bool,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct AddModelRequest {
    pub model_id: String,
    pub display_name: String,
    pub model_type: String,
}

#[derive(Clone)]
pub struct AdminConfigRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl AdminConfigRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    pub fn seed_defaults(&self) -> Result<()> {
        let conn = self.pool.get().context("Failed to get db connection")?;

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM admin_providers", [], |row| row.get(0))
            .context("Failed to count admin_providers")?;

        if count > 0 {
            return Ok(());
        }

        let now = chrono::Utc::now().to_rfc3339();
        let providers = llm_provider::supported_providers();

        for p in &providers {
            let pid = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO admin_providers (id, provider_id, display_name, enabled, supports_completion, supports_embeddings, created_at)
                 VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6)",
                params![pid, p.id, p.name, p.supports_completion, p.supports_embeddings, now],
            )
            .context("Failed to seed provider")?;

            // Seed all completion models
            for m in p.completion_models {
                let mid = Uuid::new_v4().to_string();
                let is_default = m.id == p.default_model;
                conn.execute(
                    "INSERT INTO admin_models (id, provider_id, model_id, display_name, model_type, is_default, created_at)
                     VALUES (?1, ?2, ?3, ?4, 'completion', ?5, ?6)",
                    params![mid, p.id, m.id, m.display_name, is_default as i32, now],
                )
                .context("Failed to seed completion model")?;
            }

            // Seed all embedding models
            for m in p.embedding_models {
                let mid = Uuid::new_v4().to_string();
                let is_default = p.default_embedding_model == Some(m.id);
                conn.execute(
                    "INSERT INTO admin_models (id, provider_id, model_id, display_name, model_type, is_default, created_at)
                     VALUES (?1, ?2, ?3, ?4, 'embedding', ?5, ?6)",
                    params![mid, p.id, m.id, m.display_name, is_default as i32, now],
                )
                .context("Failed to seed embedding model")?;
            }
        }

        tracing::info!("Seeded {} admin providers with default models", providers.len());
        Ok(())
    }

    pub fn list_providers(&self) -> Result<Vec<AdminProvider>> {
        let conn = self.pool.get().context("Failed to get db connection")?;
        let mut stmt = conn.prepare(
            "SELECT id, provider_id, display_name, enabled, supports_completion, supports_embeddings, created_at
             FROM admin_providers ORDER BY display_name",
        )?;

        let rows = stmt
            .query_map([], |row| {
                Ok(AdminProvider {
                    id: row.get(0)?,
                    provider_id: row.get(1)?,
                    display_name: row.get(2)?,
                    enabled: row.get::<_, i32>(3)? != 0,
                    supports_completion: row.get::<_, i32>(4)? != 0,
                    supports_embeddings: row.get::<_, i32>(5)? != 0,
                    created_at: row.get(6)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to collect providers")?;

        Ok(rows)
    }

    pub fn get_enabled_providers(&self) -> Result<Vec<AdminProvider>> {
        let all = self.list_providers()?;
        Ok(all.into_iter().filter(|p| p.enabled).collect())
    }

    pub fn toggle_provider(&self, provider_id: &str, enabled: bool) -> Result<()> {
        let conn = self.pool.get().context("Failed to get db connection")?;
        conn.execute(
            "UPDATE admin_providers SET enabled = ?1 WHERE provider_id = ?2",
            params![enabled as i32, provider_id],
        )
        .context("Failed to toggle provider")?;
        Ok(())
    }

    pub fn list_models(&self, provider_id: &str) -> Result<Vec<AdminModel>> {
        let conn = self.pool.get().context("Failed to get db connection")?;
        let mut stmt = conn.prepare(
            "SELECT id, provider_id, model_id, display_name, model_type, is_default, created_at
             FROM admin_models WHERE provider_id = ?1 ORDER BY model_type, display_name",
        )?;

        let rows = stmt
            .query_map(params![provider_id], |row| {
                Ok(AdminModel {
                    id: row.get(0)?,
                    provider_id: row.get(1)?,
                    model_id: row.get(2)?,
                    display_name: row.get(3)?,
                    model_type: row.get(4)?,
                    is_default: row.get::<_, i32>(5)? != 0,
                    created_at: row.get(6)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to collect models")?;

        Ok(rows)
    }

    pub fn get_models_by_type(&self, provider_id: &str, model_type: &str) -> Result<Vec<AdminModel>> {
        let all = self.list_models(provider_id)?;
        Ok(all.into_iter().filter(|m| m.model_type == model_type).collect())
    }

    pub fn add_model(&self, provider_id: &str, req: &AddModelRequest) -> Result<AdminModel> {
        let conn = self.pool.get().context("Failed to get db connection")?;
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO admin_models (id, provider_id, model_id, display_name, model_type, is_default, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6)",
            params![id, provider_id, req.model_id, req.display_name, req.model_type, now],
        )
        .context("Failed to add model")?;

        Ok(AdminModel {
            id,
            provider_id: provider_id.to_string(),
            model_id: req.model_id.clone(),
            display_name: req.display_name.clone(),
            model_type: req.model_type.clone(),
            is_default: false,
            created_at: now,
        })
    }

    pub fn remove_model(&self, model_id: &str) -> Result<()> {
        let conn = self.pool.get().context("Failed to get db connection")?;
        conn.execute("DELETE FROM admin_models WHERE id = ?1", params![model_id])
            .context("Failed to remove model")?;
        Ok(())
    }

    pub fn set_default_model(&self, model_id: &str) -> Result<()> {
        let conn = self.pool.get().context("Failed to get db connection")?;

        // Get model info to know provider_id and model_type
        let (provider_id, model_type): (String, String) = conn
            .query_row(
                "SELECT provider_id, model_type FROM admin_models WHERE id = ?1",
                params![model_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .context("Model not found")?;

        // Unset all defaults for this provider+type, then set the new one
        conn.execute(
            "UPDATE admin_models SET is_default = 0 WHERE provider_id = ?1 AND model_type = ?2",
            params![provider_id, model_type],
        )?;
        conn.execute(
            "UPDATE admin_models SET is_default = 1 WHERE id = ?1",
            params![model_id],
        )?;

        Ok(())
    }
}
