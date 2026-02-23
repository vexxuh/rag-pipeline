use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::services::llm_provider;

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
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
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
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
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct AddModelRequest {
    pub model_id: String,
    pub display_name: String,
    pub model_type: String,
}

#[derive(Clone)]
pub struct AdminConfigRepository {
    pool: PgPool,
}

impl AdminConfigRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn seed_defaults(&self) -> Result<()> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM admin_providers")
            .fetch_one(&self.pool)
            .await
            .context("Failed to count admin_providers")?;

        if count > 0 {
            return Ok(());
        }

        let now = chrono::Utc::now();
        let providers = llm_provider::supported_providers();

        for p in &providers {
            let pid = Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO admin_providers
                     (id, provider_id, display_name, enabled, supports_completion, supports_embeddings, created_at)
                 VALUES ($1, $2, $3, TRUE, $4, $5, $6)",
            )
            .bind(&pid)
            .bind(p.id)
            .bind(p.name)
            .bind(p.supports_completion)
            .bind(p.supports_embeddings)
            .bind(now)
            .execute(&self.pool)
            .await
            .context("Failed to seed provider")?;

            for m in p.completion_models {
                let mid = Uuid::new_v4().to_string();
                let is_default = m.id == p.default_model;
                sqlx::query(
                    "INSERT INTO admin_models
                         (id, provider_id, model_id, display_name, model_type, is_default, created_at)
                     VALUES ($1, $2, $3, $4, 'completion', $5, $6)",
                )
                .bind(&mid)
                .bind(p.id)
                .bind(m.id)
                .bind(m.display_name)
                .bind(is_default)
                .bind(now)
                .execute(&self.pool)
                .await
                .context("Failed to seed completion model")?;
            }

            for m in p.embedding_models {
                let mid = Uuid::new_v4().to_string();
                let is_default = p.default_embedding_model == Some(m.id);
                sqlx::query(
                    "INSERT INTO admin_models
                         (id, provider_id, model_id, display_name, model_type, is_default, created_at)
                     VALUES ($1, $2, $3, $4, 'embedding', $5, $6)",
                )
                .bind(&mid)
                .bind(p.id)
                .bind(m.id)
                .bind(m.display_name)
                .bind(is_default)
                .bind(now)
                .execute(&self.pool)
                .await
                .context("Failed to seed embedding model")?;
            }
        }

        tracing::info!("Seeded {} admin providers with default models", providers.len());
        Ok(())
    }

    pub async fn list_providers(&self) -> Result<Vec<AdminProvider>> {
        let rows = sqlx::query(
            "SELECT id, provider_id, display_name, enabled, supports_completion, supports_embeddings,
                    to_char(created_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at
             FROM admin_providers ORDER BY display_name",
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to list providers")?;

        let providers = rows
            .iter()
            .map(|row| AdminProvider {
                id: row.get("id"),
                provider_id: row.get("provider_id"),
                display_name: row.get("display_name"),
                enabled: row.get("enabled"),
                supports_completion: row.get("supports_completion"),
                supports_embeddings: row.get("supports_embeddings"),
                created_at: row.get("created_at"),
            })
            .collect();

        Ok(providers)
    }

    pub async fn get_enabled_providers(&self) -> Result<Vec<AdminProvider>> {
        let all = self.list_providers().await?;
        Ok(all.into_iter().filter(|p| p.enabled).collect())
    }

    pub async fn toggle_provider(&self, provider_id: &str, enabled: bool) -> Result<()> {
        sqlx::query("UPDATE admin_providers SET enabled = $1 WHERE provider_id = $2")
            .bind(enabled)
            .bind(provider_id)
            .execute(&self.pool)
            .await
            .context("Failed to toggle provider")?;

        Ok(())
    }

    pub async fn list_models(&self, provider_id: &str) -> Result<Vec<AdminModel>> {
        let rows = sqlx::query(
            "SELECT id, provider_id, model_id, display_name, model_type, is_default,
                    to_char(created_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at
             FROM admin_models WHERE provider_id = $1 ORDER BY model_type, display_name",
        )
        .bind(provider_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to list models")?;

        let models = rows
            .iter()
            .map(|row| AdminModel {
                id: row.get("id"),
                provider_id: row.get("provider_id"),
                model_id: row.get("model_id"),
                display_name: row.get("display_name"),
                model_type: row.get("model_type"),
                is_default: row.get("is_default"),
                created_at: row.get("created_at"),
            })
            .collect();

        Ok(models)
    }

    pub async fn get_models_by_type(
        &self,
        provider_id: &str,
        model_type: &str,
    ) -> Result<Vec<AdminModel>> {
        let all = self.list_models(provider_id).await?;
        Ok(all.into_iter().filter(|m| m.model_type == model_type).collect())
    }

    pub async fn add_model(&self, provider_id: &str, req: &AddModelRequest) -> Result<AdminModel> {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now();

        sqlx::query(
            "INSERT INTO admin_models (id, provider_id, model_id, display_name, model_type, is_default, created_at)
             VALUES ($1, $2, $3, $4, $5, FALSE, $6)",
        )
        .bind(&id)
        .bind(provider_id)
        .bind(&req.model_id)
        .bind(&req.display_name)
        .bind(&req.model_type)
        .bind(now)
        .execute(&self.pool)
        .await
        .context("Failed to add model")?;

        Ok(AdminModel {
            id,
            provider_id: provider_id.to_string(),
            model_id: req.model_id.clone(),
            display_name: req.display_name.clone(),
            model_type: req.model_type.clone(),
            is_default: false,
            created_at: now.to_rfc3339(),
        })
    }

    pub async fn remove_model(&self, model_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM admin_models WHERE id = $1")
            .bind(model_id)
            .execute(&self.pool)
            .await
            .context("Failed to remove model")?;

        Ok(())
    }

    pub async fn set_default_model(&self, model_id: &str) -> Result<()> {
        let row = sqlx::query(
            "SELECT provider_id, model_type FROM admin_models WHERE id = $1",
        )
        .bind(model_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to query model")?
        .ok_or_else(|| anyhow::anyhow!("Model not found"))?;

        let provider_id: String = row.get("provider_id");
        let model_type: String = row.get("model_type");

        sqlx::query(
            "UPDATE admin_models SET is_default = FALSE WHERE provider_id = $1 AND model_type = $2",
        )
        .bind(&provider_id)
        .bind(&model_type)
        .execute(&self.pool)
        .await?;

        sqlx::query("UPDATE admin_models SET is_default = TRUE WHERE id = $1")
            .bind(model_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}
