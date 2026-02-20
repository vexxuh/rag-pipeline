use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::db::models::conversation::{ConversationWithUser, Message};
use crate::errors::AppError;
use crate::middleware::auth::{require_admin, Claims};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct LogsQuery {
    pub user_id: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct LogsResponse {
    pub conversations: Vec<ConversationWithUser>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

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
pub struct LogDetailResponse {
    pub id: String,
    pub user_id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub messages: Vec<Message>,
}

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
