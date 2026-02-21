use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;

use crate::db::models::admin_config::{AddModelRequest, AdminModel, AdminProvider};
use crate::errors::AppError;
use crate::middleware::auth::{require_admin, Claims};
use crate::state::AppState;

#[cfg_attr(feature = "openapi", utoipa::path(get, path = "/api/admin/config/providers", tag = "Admin - Config", security(("bearer_auth" = [])), responses((status = 200, body = Vec<AdminProvider>))))]
pub async fn list_providers(
    State(state): State<AppState>,
    claims: Claims,
) -> Result<Json<Vec<AdminProvider>>, AppError> {
    require_admin(&claims)?;
    let providers = state.admin_config_repo.list_providers().await?;
    Ok(Json(providers))
}

#[derive(Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ToggleRequest {
    pub enabled: bool,
}

#[cfg_attr(feature = "openapi", utoipa::path(put, path = "/api/admin/config/providers/{provider_id}/toggle", tag = "Admin - Config", security(("bearer_auth" = [])), params(("provider_id" = String, Path, description = "Provider ID")), request_body = ToggleRequest, responses((status = 200))))]
pub async fn toggle_provider(
    State(state): State<AppState>,
    claims: Claims,
    Path(provider_id): Path<String>,
    Json(payload): Json<ToggleRequest>,
) -> Result<(), AppError> {
    require_admin(&claims)?;
    state
        .admin_config_repo
        .toggle_provider(&provider_id, payload.enabled)
        .await?;
    Ok(())
}

#[cfg_attr(feature = "openapi", utoipa::path(get, path = "/api/admin/config/providers/{provider_id}/models", tag = "Admin - Config", security(("bearer_auth" = [])), params(("provider_id" = String, Path, description = "Provider ID")), responses((status = 200, body = Vec<AdminModel>))))]
pub async fn list_models(
    State(state): State<AppState>,
    claims: Claims,
    Path(provider_id): Path<String>,
) -> Result<Json<Vec<AdminModel>>, AppError> {
    require_admin(&claims)?;
    let models = state.admin_config_repo.list_models(&provider_id).await?;
    Ok(Json(models))
}

#[cfg_attr(feature = "openapi", utoipa::path(post, path = "/api/admin/config/providers/{provider_id}/models", tag = "Admin - Config", security(("bearer_auth" = [])), params(("provider_id" = String, Path, description = "Provider ID")), request_body = AddModelRequest, responses((status = 200, body = AdminModel))))]
pub async fn add_model(
    State(state): State<AppState>,
    claims: Claims,
    Path(provider_id): Path<String>,
    Json(payload): Json<AddModelRequest>,
) -> Result<Json<AdminModel>, AppError> {
    require_admin(&claims)?;

    if payload.model_id.trim().is_empty() {
        return Err(AppError::Validation("Model ID is required".to_string()));
    }
    if payload.model_type != "completion" && payload.model_type != "embedding" {
        return Err(AppError::Validation(
            "model_type must be 'completion' or 'embedding'".to_string(),
        ));
    }

    let model = state
        .admin_config_repo
        .add_model(&provider_id, &payload)
        .await?;
    Ok(Json(model))
}

#[cfg_attr(feature = "openapi", utoipa::path(delete, path = "/api/admin/config/models/{model_id}", tag = "Admin - Config", security(("bearer_auth" = [])), params(("model_id" = String, Path, description = "Model ID")), responses((status = 200))))]
pub async fn remove_model(
    State(state): State<AppState>,
    claims: Claims,
    Path(model_id): Path<String>,
) -> Result<(), AppError> {
    require_admin(&claims)?;
    state.admin_config_repo.remove_model(&model_id).await?;
    Ok(())
}

#[cfg_attr(feature = "openapi", utoipa::path(put, path = "/api/admin/config/models/{model_id}/default", tag = "Admin - Config", security(("bearer_auth" = [])), params(("model_id" = String, Path, description = "Model ID")), responses((status = 200))))]
pub async fn set_default_model(
    State(state): State<AppState>,
    claims: Claims,
    Path(model_id): Path<String>,
) -> Result<(), AppError> {
    require_admin(&claims)?;
    state.admin_config_repo.set_default_model(&model_id).await?;
    Ok(())
}
