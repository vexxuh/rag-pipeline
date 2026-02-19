use anyhow::Context;
use axum::{middleware as axum_mw, routing::get, Router};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use rag_backend::config::AppConfig;
use rag_backend::db::{connection, migrations};
use rag_backend::middleware::auth::auth_middleware;
use rag_backend::routes::health;
use rag_backend::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let config = AppConfig::load().context("Failed to load configuration")?;
    tracing::info!("Configuration loaded (env: {})", std::env::var("RUN_ENV").unwrap_or_else(|_| "development".into()));

    let db_pool =
        connection::create_pool(&config.database).context("Failed to create database pool")?;

    {
        let conn = db_pool
            .get()
            .context("Failed to get connection for migrations")?;
        migrations::run_all(&conn).context("Failed to run migrations")?;
    }

    let state = AppState::new(config.clone(), db_pool);

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let public_routes = Router::new().route("/api/health", get(health::health_check));

    let protected_routes = Router::new()
        // Protected routes will be added in subsequent phases
        .layer(axum_mw::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    let app = Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = format!("{}:{}", config.server.host, config.server.port);
    tracing::info!("Starting server on {addr}");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .context("Failed to bind to address")?;

    axum::serve(listener, app)
        .await
        .context("Server error")?;

    Ok(())
}
