use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;

use crate::db::models::admin_config::{AdminModel, AdminProvider};
use crate::db::models::settings::{ApiKeyEntry, LlmPreferences};
use crate::errors::AppError;
use crate::middleware::auth::Claims;
use crate::services::audit;
use crate::state::AppState;

// ── Providers (user-facing, only admin-enabled) ─────────────
pub async fn list_providers(
    State(state): State<AppState>,
) -> Result<Json<Vec<AdminProvider>>, AppError> {
    let providers = state.admin_config_repo.get_enabled_providers().await?;
    Ok(Json(providers))
}

pub async fn list_models_for_provider(
    State(state): State<AppState>,
    Path(provider_id): Path<String>,
) -> Result<Json<Vec<AdminModel>>, AppError> {
    let models = state.admin_config_repo.list_models(&provider_id).await?;
    Ok(Json(models))
}

// ── API Keys ─────────────────────────────────────────────────
#[derive(Debug, Deserialize)]
pub struct SetApiKeyRequest {
    pub api_key: String,
}

pub async fn list_api_keys(
    State(state): State<AppState>,
    claims: Claims,
) -> Result<Json<Vec<ApiKeyEntry>>, AppError> {
    let keys = state.settings_repo.list_api_keys(&claims.sub).await?;
    Ok(Json(keys))
}

pub async fn set_api_key(
    State(state): State<AppState>,
    claims: Claims,
    Path(provider): Path<String>,
    Json(payload): Json<SetApiKeyRequest>,
) -> Result<Json<ApiKeyEntry>, AppError> {
    if payload.api_key.trim().is_empty() {
        return Err(AppError::Validation("API key cannot be empty".to_string()));
    }

    let entry = state
        .settings_repo
        .set_api_key(&claims.sub, &provider, &payload.api_key).await?;

    audit::log(
        &state.audit_log_repo,
        Some(&claims.sub),
        "settings.update_key",
        Some("api_key"),
        Some(&provider),
        &format!("Updated API key for provider '{provider}'"),
        None,
        None,
    );

    Ok(Json(entry))
}

pub async fn delete_api_key(
    State(state): State<AppState>,
    claims: Claims,
    Path(provider): Path<String>,
) -> Result<(), AppError> {
    state.settings_repo.delete_api_key(&claims.sub, &provider).await?;

    audit::log(
        &state.audit_log_repo,
        Some(&claims.sub),
        "settings.delete_key",
        Some("api_key"),
        Some(&provider),
        &format!("Deleted API key for provider '{provider}'"),
        None,
        None,
    );

    Ok(())
}

// ── LLM Preferences ─────────────────────────────────────────
pub async fn get_preferences(
    State(state): State<AppState>,
    claims: Claims,
) -> Result<Json<LlmPreferences>, AppError> {
    let prefs = state
        .settings_repo
        .get_preferences(&claims.sub).await?
        .unwrap_or_else(|| LlmPreferences {
            preferred_provider: state.config.llm.default_provider.clone(),
            preferred_model: state.config.llm.default_model.clone(),
            preferred_embedding_model: state.config.llm.default_embedding_model.clone(),
            system_prompt: state.config.llm.default_system_prompt.clone(),
        });

    Ok(Json(prefs))
}

pub async fn update_preferences(
    State(state): State<AppState>,
    claims: Claims,
    Json(payload): Json<LlmPreferences>,
) -> Result<Json<LlmPreferences>, AppError> {
    state
        .settings_repo
        .set_preferences(&claims.sub, &payload).await?;

    audit::log(
        &state.audit_log_repo,
        Some(&claims.sub),
        "settings.update_preferences",
        None,
        None,
        "Updated LLM preferences",
        None,
        None,
    );

    Ok(Json(payload))
}
