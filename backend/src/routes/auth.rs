use axum::{extract::State, Json};
use axum::http::HeaderMap;

use crate::dto::auth::{AuthResponse, LoginRequest, SetupRequest, UserResponse};
use crate::errors::AppError;
use crate::middleware::auth::{extract_ip, Claims};
use crate::services::{audit, auth_service};
use crate::state::AppState;

pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    let user = state
        .user_repo
        .find_by_email(&payload.email)
        .await?
        .ok_or_else(|| AppError::Validation("Invalid email or password".to_string()))?;

    let valid = auth_service::verify_password(&payload.password, &user.password_hash)
        .map_err(AppError::Internal)?;

    if !valid {
        return Err(AppError::Validation(
            "Invalid email or password".to_string(),
        ));
    }

    let token = auth_service::generate_jwt(
        &user.id,
        &user.username,
        &user.role.to_string(),
        &state.config.auth,
    )
    .map_err(AppError::Internal)?;

    let ip = extract_ip(&headers);
    audit::log(
        &state.audit_log_repo,
        Some(&user.id),
        "auth.login",
        None,
        None,
        &format!("User '{}' logged in", user.username),
        ip.as_deref(),
        None,
    );

    Ok(Json(AuthResponse {
        token,
        user: user.into(),
    }))
}

pub async fn setup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<SetupRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    validate_setup(&payload)?;

    let invite = state
        .invite_repo
        .find_by_token(&payload.token)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::Validation("Invalid invite token".to_string()))?;

    if invite.used {
        return Err(AppError::Validation(
            "This invite has already been used".to_string(),
        ));
    }

    let expires = chrono::DateTime::parse_from_rfc3339(&invite.expires_at)
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid expiry date")))?;

    if chrono::Utc::now() > expires {
        return Err(AppError::Validation("This invite has expired".to_string()));
    }

    if state.user_repo.find_by_email(&invite.email).await?.is_some() {
        return Err(AppError::Validation(
            "An account with this email already exists".to_string(),
        ));
    }

    let password_hash =
        auth_service::hash_password(&payload.password).map_err(AppError::Internal)?;

    let role = invite.role.clone();
    let user = state
        .user_repo
        .create(&payload.username, &invite.email, &password_hash, &role)
        .await?;

    state
        .invite_repo
        .mark_used(&payload.token)
        .await
        .map_err(AppError::Internal)?;

    let token = auth_service::generate_jwt(
        &user.id,
        &user.username,
        &user.role.to_string(),
        &state.config.auth,
    )
    .map_err(AppError::Internal)?;

    let ip = extract_ip(&headers);
    audit::log(
        &state.audit_log_repo,
        Some(&user.id),
        "auth.setup",
        None,
        None,
        &format!("User '{}' completed account setup", user.username),
        ip.as_deref(),
        None,
    );

    Ok(Json(AuthResponse {
        token,
        user: user.into(),
    }))
}

pub async fn me(
    State(state): State<AppState>,
    claims: Claims,
) -> Result<Json<UserResponse>, AppError> {
    let user = state
        .user_repo
        .find_by_id(&claims.sub)
        .await?
        .ok_or(AppError::NotFound("User not found".to_string()))?;

    Ok(Json(user.into()))
}

fn validate_setup(req: &SetupRequest) -> Result<(), AppError> {
    if req.username.trim().is_empty() || req.username.len() < 3 {
        return Err(AppError::Validation(
            "Username must be at least 3 characters".to_string(),
        ));
    }
    if req.password.len() < 8 {
        return Err(AppError::Validation(
            "Password must be at least 8 characters".to_string(),
        ));
    }
    Ok(())
}
