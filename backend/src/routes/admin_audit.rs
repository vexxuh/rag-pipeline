use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};

use crate::db::models::audit_log::AuditLog;
use crate::errors::AppError;
use crate::middleware::auth::{require_admin, Claims};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct AuditLogsQuery {
    pub user_id: Option<String>,
    pub event_type: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct AuditLogsResponse {
    pub logs: Vec<AuditLog>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

pub async fn list_audit_logs(
    State(state): State<AppState>,
    claims: Claims,
    axum::extract::Query(query): axum::extract::Query<AuditLogsQuery>,
) -> Result<Json<AuditLogsResponse>, AppError> {
    require_admin(&claims)?;

    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(50).clamp(1, 100);
    let offset = (page - 1) * per_page;

    let total = state
        .audit_log_repo
        .count(
            query.user_id.as_deref(),
            query.event_type.as_deref(),
            query.from.as_deref(),
            query.to.as_deref(),
        )
        .await?;

    let logs = state
        .audit_log_repo
        .list(
            query.user_id.as_deref(),
            query.event_type.as_deref(),
            query.from.as_deref(),
            query.to.as_deref(),
            per_page,
            offset,
        )
        .await?;

    Ok(Json(AuditLogsResponse {
        logs,
        total,
        page,
        per_page,
    }))
}
