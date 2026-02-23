use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::db::models::conversation::{ConversationWithUser, Message, WidgetConversationLog};
use crate::errors::AppError;
use crate::middleware::auth::{require_admin, Claims};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::IntoParams))]
pub struct LogsQuery {
    pub user_id: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct LogsResponse {
    pub conversations: Vec<ConversationWithUser>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

#[cfg_attr(feature = "openapi", utoipa::path(get, path = "/api/admin/logs", tag = "Admin - Logs", security(("bearer_auth" = [])), params(LogsQuery), responses((status = 200, body = LogsResponse))))]
pub async fn list_conversation_logs(
    State(state): State<AppState>,
    claims: Claims,
    Query(query): Query<LogsQuery>,
) -> Result<Json<LogsResponse>, AppError> {
    require_admin(&claims)?;

    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(25).clamp(1, 100);
    let offset = (page - 1) * per_page;
    let user_id_filter = query.user_id.as_deref();

    let total = state
        .conversation_repo
        .count_all(user_id_filter)
        .await?;
    let conversations = state
        .conversation_repo
        .list_all(user_id_filter, per_page, offset)
        .await?;

    Ok(Json(LogsResponse {
        conversations,
        total,
        page,
        per_page,
    }))
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct LogDetailResponse {
    pub id: String,
    pub user_id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub messages: Vec<Message>,
}

#[cfg_attr(feature = "openapi", utoipa::path(get, path = "/api/admin/logs/{id}", tag = "Admin - Logs", security(("bearer_auth" = [])), params(("id" = String, Path, description = "Conversation ID")), responses((status = 200, body = LogDetailResponse))))]
pub async fn get_conversation_log(
    State(state): State<AppState>,
    claims: Claims,
    Path(id): Path<String>,
) -> Result<Json<LogDetailResponse>, AppError> {
    require_admin(&claims)?;

    let conv = state
        .conversation_repo
        .get_by_id(&id)
        .await?
        .ok_or_else(|| AppError::NotFound("Conversation not found".to_string()))?;

    let messages = state.conversation_repo.get_messages(&id).await?;

    Ok(Json(LogDetailResponse {
        id: conv.id,
        user_id: conv.user_id,
        title: conv.title,
        created_at: conv.created_at,
        updated_at: conv.updated_at,
        messages,
    }))
}

// ── Widget logs ──────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::IntoParams))]
pub struct WidgetLogsQuery {
    pub embed_key_id: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct WidgetLogsResponse {
    pub conversations: Vec<WidgetConversationLog>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

#[cfg_attr(feature = "openapi", utoipa::path(get, path = "/api/admin/widget-logs", tag = "Admin - Logs", security(("bearer_auth" = [])), params(WidgetLogsQuery), responses((status = 200, body = WidgetLogsResponse))))]
pub async fn list_widget_logs(
    State(state): State<AppState>,
    claims: Claims,
    Query(query): Query<WidgetLogsQuery>,
) -> Result<Json<WidgetLogsResponse>, AppError> {
    require_admin(&claims)?;

    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(25).clamp(1, 100);
    let offset = (page - 1) * per_page;
    let embed_key_id_filter = query.embed_key_id.as_deref();

    let total = state
        .conversation_repo
        .count_widget_conversations(embed_key_id_filter)
        .await?;
    let conversations = state
        .conversation_repo
        .list_widget_conversations(embed_key_id_filter, per_page, offset)
        .await?;

    Ok(Json(WidgetLogsResponse {
        conversations,
        total,
        page,
        per_page,
    }))
}
