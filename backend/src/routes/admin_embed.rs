use axum::{
    extract::{Path, State},
    Json,
};
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::db::models::embed_key::{EmbedKey, UpdateEmbedKeyRequest};
use crate::errors::AppError;
use crate::middleware::auth::{require_admin, Claims};
use crate::middleware::embed_auth::hash_key;
use crate::services::audit;
use crate::state::AppState;

#[derive(Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct CreateEmbedKeyRequest {
    pub name: String,
    #[serde(default)]
    pub allowed_domains: Vec<String>,
    #[serde(default)]
    pub system_prompt: String,
    pub rate_limit: Option<i32>,
    #[serde(default = "default_widget_title")]
    pub widget_title: String,
    #[serde(default = "default_primary_color")]
    pub primary_color: String,
    #[serde(default = "default_greeting")]
    pub greeting_message: String,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub api_key: String,
}

fn default_widget_title() -> String {
    "Chat with us".to_string()
}
fn default_primary_color() -> String {
    "#2563eb".to_string()
}
fn default_greeting() -> String {
    "Hello! How can I help you?".to_string()
}

#[derive(Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct CreateEmbedKeyResponse {
    pub embed_key: EmbedKey,
    pub raw_key: String,
}

#[cfg_attr(feature = "openapi", utoipa::path(post, path = "/api/admin/embed-keys", tag = "Admin - Embed", security(("bearer_auth" = [])), request_body = CreateEmbedKeyRequest, responses((status = 200, body = CreateEmbedKeyResponse))))]
pub async fn create_key(
    State(state): State<AppState>,
    claims: Claims,
    Json(payload): Json<CreateEmbedKeyRequest>,
) -> Result<Json<CreateEmbedKeyResponse>, AppError> {
    require_admin(&claims)?;

    if payload.name.trim().is_empty() {
        return Err(AppError::Validation("Name is required".to_string()));
    }

    // Generate cryptographically random key (scoped to avoid Send issue)
    let raw_key = {
        let mut rng = rand::rng();
        let mut key_bytes = [0u8; 32];
        rng.fill(&mut key_bytes);
        format!(
            "ek_{}",
            key_bytes
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        )
    };

    let key_hash = hash_key(&raw_key);
    let key_prefix = &raw_key[..11.min(raw_key.len())]; // "ek_" + first 8 hex chars

    let id = uuid::Uuid::new_v4().to_string();
    let rate_limit = payload
        .rate_limit
        .unwrap_or(state.config.widget.default_rate_limit);

    let embed_key = state
        .embed_key_repo
        .create(
            &id,
            payload.name.trim(),
            &key_hash,
            key_prefix,
            &payload.allowed_domains,
            &payload.system_prompt,
            rate_limit,
            &payload.widget_title,
            &payload.primary_color,
            &payload.greeting_message,
            &payload.provider,
            &payload.model,
            &payload.api_key,
        )
        .await?;

    audit::log(
        &state.audit_log_repo,
        Some(&claims.sub),
        "admin.embed_key.create",
        Some("embed_key"),
        Some(&id),
        &format!("Created embed key '{}'", payload.name.trim()),
        None,
        None,
    );

    Ok(Json(CreateEmbedKeyResponse {
        embed_key,
        raw_key,
    }))
}

#[cfg_attr(feature = "openapi", utoipa::path(get, path = "/api/admin/embed-keys", tag = "Admin - Embed", security(("bearer_auth" = [])), responses((status = 200, body = Vec<EmbedKey>))))]
pub async fn list_keys(
    State(state): State<AppState>,
    claims: Claims,
) -> Result<Json<Vec<EmbedKey>>, AppError> {
    require_admin(&claims)?;
    let keys = state.embed_key_repo.list_all().await?;
    Ok(Json(keys))
}

#[cfg_attr(feature = "openapi", utoipa::path(get, path = "/api/admin/embed-keys/{id}", tag = "Admin - Embed", security(("bearer_auth" = [])), params(("id" = String, Path, description = "Embed key ID")), responses((status = 200, body = EmbedKey))))]
pub async fn get_key(
    State(state): State<AppState>,
    claims: Claims,
    Path(id): Path<String>,
) -> Result<Json<EmbedKey>, AppError> {
    require_admin(&claims)?;
    let key = state
        .embed_key_repo
        .find_by_id(&id)
        .await?
        .ok_or_else(|| AppError::NotFound("Embed key not found".to_string()))?;
    Ok(Json(key))
}

#[cfg_attr(feature = "openapi", utoipa::path(put, path = "/api/admin/embed-keys/{id}", tag = "Admin - Embed", security(("bearer_auth" = [])), params(("id" = String, Path, description = "Embed key ID")), request_body = UpdateEmbedKeyRequest, responses((status = 200, body = EmbedKey))))]
pub async fn update_key(
    State(state): State<AppState>,
    claims: Claims,
    Path(id): Path<String>,
    Json(payload): Json<UpdateEmbedKeyRequest>,
) -> Result<Json<EmbedKey>, AppError> {
    require_admin(&claims)?;

    let key = state
        .embed_key_repo
        .update(&id, &payload)
        .await?
        .ok_or_else(|| AppError::NotFound("Embed key not found".to_string()))?;

    audit::log(
        &state.audit_log_repo,
        Some(&claims.sub),
        "admin.embed_key.update",
        Some("embed_key"),
        Some(&id),
        &format!("Updated embed key '{}'", key.name),
        None,
        None,
    );

    Ok(Json(key))
}

#[cfg_attr(feature = "openapi", utoipa::path(delete, path = "/api/admin/embed-keys/{id}", tag = "Admin - Embed", security(("bearer_auth" = [])), params(("id" = String, Path, description = "Embed key ID")), responses((status = 200))))]
pub async fn delete_key(
    State(state): State<AppState>,
    claims: Claims,
    Path(id): Path<String>,
) -> Result<(), AppError> {
    require_admin(&claims)?;

    state.embed_key_repo.delete(&id).await?;

    audit::log(
        &state.audit_log_repo,
        Some(&claims.sub),
        "admin.embed_key.delete",
        Some("embed_key"),
        Some(&id),
        "Deleted embed key",
        None,
        None,
    );

    Ok(())
}

#[cfg_attr(feature = "openapi", utoipa::path(put, path = "/api/admin/embed-keys/{id}/toggle", tag = "Admin - Embed", security(("bearer_auth" = [])), params(("id" = String, Path, description = "Embed key ID")), responses((status = 200, body = EmbedKey))))]
pub async fn toggle_key(
    State(state): State<AppState>,
    claims: Claims,
    Path(id): Path<String>,
) -> Result<Json<EmbedKey>, AppError> {
    require_admin(&claims)?;

    let key = state
        .embed_key_repo
        .toggle(&id)
        .await?
        .ok_or_else(|| AppError::NotFound("Embed key not found".to_string()))?;

    let action = if key.is_active {
        "Activated"
    } else {
        "Deactivated"
    };

    audit::log(
        &state.audit_log_repo,
        Some(&claims.sub),
        "admin.embed_key.toggle",
        Some("embed_key"),
        Some(&id),
        &format!("{action} embed key '{}'", key.name),
        None,
        None,
    );

    Ok(Json(key))
}
