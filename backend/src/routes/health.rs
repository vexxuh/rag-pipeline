use axum::Json;
use serde::Serialize;

#[derive(Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

#[cfg_attr(feature = "openapi", utoipa::path(get, path = "/api/health", tag = "Health", responses((status = 200, body = HealthResponse))))]
pub async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}
