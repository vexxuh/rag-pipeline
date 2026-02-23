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

    // --- Domain validation ---
    // Extract origin from Origin header, falling back to Referer.
    let origin = req
        .headers()
        .get("origin")
        .or_else(|| req.headers().get("referer"))
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    if embed_key.allowed_domains.is_empty() {
        // No domains configured = BLOCK all browser requests.
        // Only requests without an Origin/Referer (e.g. server-side, curl) pass through.
        // This is a secure default: admins must explicitly list allowed domains.
        if !origin.is_empty() {
            tracing::warn!(
                embed_key_id = %embed_key.id,
                origin = %origin,
                "Widget request blocked: no allowed domains configured"
            );
            return Err(StatusCode::FORBIDDEN);
        }
    } else {
        // Domains are configured — validate the origin.
        if origin.is_empty() {
            // No Origin/Referer header present. Block by default because legitimate
            // browser widget requests always send an Origin header for cross-origin
            // fetch/XHR. Missing Origin likely means the request is being replayed
            // outside a browser or from a non-allowed context.
            tracing::warn!(
                embed_key_id = %embed_key.id,
                "Widget request blocked: no Origin or Referer header"
            );
            return Err(StatusCode::FORBIDDEN);
        }

        if !is_domain_allowed(&origin, &embed_key.allowed_domains) {
            tracing::warn!(
                embed_key_id = %embed_key.id,
                origin = %origin,
                "Widget request blocked: domain not in allowed list"
            );
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
        let domain = domain.trim().to_lowercase();
        if domain.is_empty() {
            return false;
        }
        // Exact match or subdomain match (e.g. "app.example.com" matches "example.com")
        origin_host == domain || origin_host.ends_with(&format!(".{domain}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_domain_match() {
        let allowed = vec!["example.com".to_string()];
        assert!(is_domain_allowed("https://example.com", &allowed));
        assert!(is_domain_allowed("https://example.com/path", &allowed));
        assert!(is_domain_allowed("http://example.com", &allowed));
    }

    #[test]
    fn test_subdomain_match() {
        let allowed = vec!["example.com".to_string()];
        assert!(is_domain_allowed("https://app.example.com", &allowed));
        assert!(is_domain_allowed("https://sub.app.example.com", &allowed));
    }

    #[test]
    fn test_rejects_similar_domains() {
        let allowed = vec!["example.com".to_string()];
        assert!(!is_domain_allowed("https://evilexample.com", &allowed));
        assert!(!is_domain_allowed("https://notexample.com", &allowed));
        assert!(!is_domain_allowed("https://example.com.evil.com", &allowed));
    }

    #[test]
    fn test_localhost_only_if_configured() {
        let allowed = vec!["localhost".to_string()];
        assert!(is_domain_allowed("http://localhost", &allowed));
        assert!(is_domain_allowed("http://localhost:3000", &allowed));
        assert!(is_domain_allowed("http://localhost:8080", &allowed));
        assert!(!is_domain_allowed("https://example.com", &allowed));
    }

    #[test]
    fn test_localhost_blocked_when_not_configured() {
        let allowed = vec!["example.com".to_string()];
        assert!(!is_domain_allowed("http://localhost", &allowed));
        assert!(!is_domain_allowed("http://localhost:3000", &allowed));
    }

    #[test]
    fn test_rejects_invalid_origins() {
        let allowed = vec!["example.com".to_string()];
        assert!(!is_domain_allowed("not-a-url", &allowed));
        assert!(!is_domain_allowed("", &allowed));
    }

    #[test]
    fn test_case_insensitive() {
        let allowed = vec!["Example.COM".to_string()];
        assert!(is_domain_allowed("https://example.com", &allowed));
        assert!(is_domain_allowed("https://EXAMPLE.COM", &allowed));
    }

    #[test]
    fn test_empty_domain_in_list_ignored() {
        let allowed = vec!["".to_string(), "example.com".to_string()];
        assert!(is_domain_allowed("https://example.com", &allowed));
        assert!(!is_domain_allowed("https://other.com", &allowed));
    }

    #[test]
    fn test_multiple_domains() {
        let allowed = vec![
            "example.com".to_string(),
            "mysite.org".to_string(),
            "localhost".to_string(),
        ];
        assert!(is_domain_allowed("https://example.com", &allowed));
        assert!(is_domain_allowed("https://mysite.org", &allowed));
        assert!(is_domain_allowed("http://localhost:8080", &allowed));
        assert!(!is_domain_allowed("https://other.com", &allowed));
    }
}
