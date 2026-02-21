use axum::{
    extract::{Path, State},
    Json,
};

use crate::db::models::invite::Invite;
use crate::dto::auth::{InviteRequest, InviteResponse, UpdateRoleRequest, UserResponse};
use crate::errors::AppError;
use crate::middleware::auth::{require_admin, Claims};
use crate::services::audit;
use crate::state::AppState;

#[cfg_attr(feature = "openapi", utoipa::path(get, path = "/api/admin/users", tag = "Admin - Users", security(("bearer_auth" = [])), responses((status = 200, body = Vec<UserResponse>))))]
pub async fn list_users(
    State(state): State<AppState>,
    claims: Claims,
) -> Result<Json<Vec<UserResponse>>, AppError> {
    require_admin(&claims)?;

    let users = state.user_repo.find_all().await?;
    let responses: Vec<UserResponse> = users.into_iter().map(|u| u.into()).collect();
    Ok(Json(responses))
}

#[cfg_attr(feature = "openapi", utoipa::path(put, path = "/api/admin/users/{user_id}/role", tag = "Admin - Users", security(("bearer_auth" = [])), params(("user_id" = String, Path, description = "User ID")), request_body = UpdateRoleRequest, responses((status = 200, body = UserResponse))))]
pub async fn update_user_role(
    State(state): State<AppState>,
    claims: Claims,
    Path(user_id): Path<String>,
    Json(payload): Json<UpdateRoleRequest>,
) -> Result<Json<UserResponse>, AppError> {
    require_admin(&claims)?;

    if claims.sub == user_id {
        return Err(AppError::Validation(
            "Cannot change your own role".to_string(),
        ));
    }

    state.user_repo.update_role(&user_id, &payload.role).await?;

    audit::log(
        &state.audit_log_repo,
        Some(&claims.sub),
        "admin.update_role",
        Some("user"),
        Some(&user_id),
        &format!("Updated user role to '{}'", payload.role),
        None,
        None,
    );

    let user = state
        .user_repo
        .find_by_id(&user_id)
        .await?
        .ok_or(AppError::NotFound("User not found".to_string()))?;

    Ok(Json(user.into()))
}

#[cfg_attr(feature = "openapi", utoipa::path(delete, path = "/api/admin/users/{user_id}", tag = "Admin - Users", security(("bearer_auth" = [])), params(("user_id" = String, Path, description = "User ID")), responses((status = 200))))]
pub async fn delete_user(
    State(state): State<AppState>,
    claims: Claims,
    Path(user_id): Path<String>,
) -> Result<(), AppError> {
    require_admin(&claims)?;

    if claims.sub == user_id {
        return Err(AppError::Validation(
            "Cannot delete your own account".to_string(),
        ));
    }

    state
        .user_repo
        .find_by_id(&user_id)
        .await?
        .ok_or(AppError::NotFound("User not found".to_string()))?;

    state.user_repo.delete(&user_id).await?;

    audit::log(
        &state.audit_log_repo,
        Some(&claims.sub),
        "admin.delete_user",
        Some("user"),
        Some(&user_id),
        "Deleted user",
        None,
        None,
    );

    Ok(())
}

#[cfg_attr(feature = "openapi", utoipa::path(post, path = "/api/admin/invites", tag = "Admin - Users", security(("bearer_auth" = [])), request_body = InviteRequest, responses((status = 200, body = InviteResponse))))]
pub async fn invite_user(
    State(state): State<AppState>,
    claims: Claims,
    Json(payload): Json<InviteRequest>,
) -> Result<Json<InviteResponse>, AppError> {
    require_admin(&claims)?;

    if payload.email.trim().is_empty() || !payload.email.contains('@') {
        return Err(AppError::Validation("Valid email is required".to_string()));
    }

    if state.user_repo.find_by_email(&payload.email).await?.is_some() {
        return Err(AppError::Validation(
            "A user with this email already exists".to_string(),
        ));
    }

    let invite = state
        .invite_repo
        .create(&payload.email, &payload.role, &claims.sub, 48)
        .await
        .map_err(AppError::Internal)?;

    audit::log(
        &state.audit_log_repo,
        Some(&claims.sub),
        "admin.invite",
        Some("invite"),
        Some(&invite.id),
        &format!("Invited '{}' with role '{}'", invite.email, invite.role),
        None,
        None,
    );

    // Send invite email (non-blocking - don't fail the request if email fails)
    let email_service = state.email.clone();
    let email = invite.email.clone();
    let token = invite.token.clone();
    tokio::spawn(async move {
        if let Err(e) = email_service.send_invite(&email, &token).await {
            tracing::error!("Failed to send invite email to {email}: {e}");
        }
    });

    let frontend_url = &state.config.resend.frontend_url;
    Ok(Json(invite_to_response(invite, frontend_url)))
}

#[cfg_attr(feature = "openapi", utoipa::path(get, path = "/api/admin/invites", tag = "Admin - Users", security(("bearer_auth" = [])), responses((status = 200, body = Vec<InviteResponse>))))]
pub async fn list_invites(
    State(state): State<AppState>,
    claims: Claims,
) -> Result<Json<Vec<InviteResponse>>, AppError> {
    require_admin(&claims)?;

    let frontend_url = state.config.resend.frontend_url.clone();
    let invites = state
        .invite_repo
        .find_all()
        .await
        .map_err(AppError::Internal)?;
    let responses: Vec<InviteResponse> = invites
        .into_iter()
        .map(|i| invite_to_response(i, &frontend_url))
        .collect();
    Ok(Json(responses))
}

fn invite_to_response(invite: Invite, frontend_url: &str) -> InviteResponse {
    let setup_link = format!("{}/setup?token={}", frontend_url, invite.token);
    InviteResponse {
        id: invite.id,
        email: invite.email,
        role: invite.role,
        used: invite.used,
        setup_link,
        expires_at: invite.expires_at,
        created_at: invite.created_at,
    }
}
