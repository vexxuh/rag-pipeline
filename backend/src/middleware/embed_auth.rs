use axum::{
    extract::{FromRequestParts, Request, State},
    http::{request::Parts, StatusCode},
    middleware::Next,
    response::Response,
};
use sha2::{Digest, Sha256};

use crate::db::models::embed_key::EmbedKey;
use crate::state::AppState;

#[derive(Debug, Clone)]
pub struct EmbedContext {
    pub embed_key: EmbedKey,
    pub session_id: String,
}

impl<S: Send + Sync> FromRequestParts<S> for EmbedContext {
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<EmbedContext>()
            .cloned()
            .ok_or(StatusCode::UNAUTHORIZED)
    }
}

pub async fn embed_auth_middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if !state.config.widget.enabled {
        return Err(StatusCode::NOT_FOUND);
    }

    // Extract embed key from header
    let raw_key = req
        .headers()
        .get("x-embed-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Hash the key
    let key_hash = hash_key(&raw_key);

    // Look up in database
    let embed_key = state
        .embed_key_repo
        .find_by_hash(&key_hash)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;

    if !embed_key.is_active {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Validate domain if allowed_domains is non-empty
    if !embed_key.allowed_domains.is_empty() {
        let origin = req
            .headers()
            .get("origin")
            .or_else(|| req.headers().get("referer"))
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        if !origin.is_empty() && !is_domain_allowed(origin, &embed_key.allowed_domains) {
            return Err(StatusCode::FORBIDDEN);
        }
    }

    // Extract or default session ID
    let session_id = req
        .headers()
        .get("x-session-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    req.extensions_mut().insert(EmbedContext {
        embed_key,
        session_id,
    });

    Ok(next.run(req).await)
}

pub fn hash_key(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn is_domain_allowed(origin: &str, allowed: &[String]) -> bool {
    let origin_host = url::Url::parse(origin)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_lowercase()));

    let origin_host = match origin_host {
        Some(h) => h,
        None => return false,
    };

    allowed.iter().any(|domain| {
        let domain = domain.to_lowercase();
        origin_host == domain || origin_host.ends_with(&format!(".{domain}"))
    })
}
