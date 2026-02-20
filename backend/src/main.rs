use anyhow::Context;
use axum::{
    middleware as axum_mw,
    routing::{delete, get, post, put},
    Router,
};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use rag_backend::config::AppConfig;
use rag_backend::db::models::user::UserRole;
use rag_backend::db::{connection, migrations};
use rag_backend::middleware::auth::auth_middleware;
use rag_backend::routes::{admin, admin_config, auth, chat, crawl, documents, health, settings};
use rag_backend::services::auth_service;
use rag_backend::services::storage::StorageService;
use rag_backend::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let config = AppConfig::load().context("Failed to load configuration")?;
    tracing::info!(
        "Configuration loaded (env: {})",
        std::env::var("RUN_ENV").unwrap_or_else(|_| "development".into())
    );

    let db_pool =
        connection::create_pool(&config.database).context("Failed to create database pool")?;

    {
        let conn = db_pool
            .get()
            .context("Failed to get connection for migrations")?;
        migrations::run_all(&conn).context("Failed to run migrations")?;
    }

    let storage = StorageService::new(&config.minio)
        .await
        .context("Failed to initialize MinIO storage")?;
    tracing::info!("MinIO storage initialized");

    let state = AppState::new(config.clone(), db_pool, storage);

    // Seed admin account on first boot
    seed_admin(&state)?;

    // Seed default provider/model catalogue
    state
        .admin_config_repo
        .seed_defaults()
        .context("Failed to seed admin config defaults")?;

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let public_routes = Router::new()
        .route("/api/health", get(health::health_check))
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/setup", post(auth::setup));

    let protected_routes = Router::new()
        // Auth
        .route("/api/auth/me", get(auth::me))
        // Conversations
        .route("/api/conversations", post(chat::create_conversation))
        .route("/api/conversations", get(chat::list_conversations))
        .route("/api/conversations/{id}", get(chat::get_conversation))
        .route("/api/conversations/{id}", delete(chat::delete_conversation))
        .route(
            "/api/conversations/{id}/messages",
            post(chat::send_message),
        )
        // Documents
        .route("/api/documents", post(documents::upload))
        .route("/api/documents", get(documents::list))
        .route("/api/documents/{id}", get(documents::get_document))
        .route("/api/documents/{id}", delete(documents::delete_document))
        // Crawl
        .route("/api/crawl", post(crawl::start_crawl))
        .route("/api/crawl", get(crawl::list_crawl_jobs))
        .route("/api/crawl/{id}", get(crawl::get_crawl_job))
        // Settings (user-facing — only admin-enabled providers/models)
        .route("/api/settings/providers", get(settings::list_providers))
        .route(
            "/api/settings/providers/{provider_id}/models",
            get(settings::list_models_for_provider),
        )
        .route("/api/settings/api-keys", get(settings::list_api_keys))
        .route(
            "/api/settings/api-keys/{provider}",
            put(settings::set_api_key),
        )
        .route(
            "/api/settings/api-keys/{provider}",
            delete(settings::delete_api_key),
        )
        .route("/api/settings/preferences", get(settings::get_preferences))
        .route(
            "/api/settings/preferences",
            put(settings::update_preferences),
        )
        // Admin — User management
        .route("/api/admin/users", get(admin::list_users))
        .route(
            "/api/admin/users/{user_id}/role",
            put(admin::update_user_role),
        )
        .route(
            "/api/admin/users/{user_id}",
            delete(admin::delete_user),
        )
        .route("/api/admin/invites", post(admin::invite_user))
        .route("/api/admin/invites", get(admin::list_invites))
        // Admin — Provider / model config
        .route(
            "/api/admin/config/providers",
            get(admin_config::list_providers),
        )
        .route(
            "/api/admin/config/providers/{provider_id}/toggle",
            put(admin_config::toggle_provider),
        )
        .route(
            "/api/admin/config/providers/{provider_id}/models",
            get(admin_config::list_models),
        )
        .route(
            "/api/admin/config/providers/{provider_id}/models",
            post(admin_config::add_model),
        )
        .route(
            "/api/admin/config/models/{model_id}",
            delete(admin_config::remove_model),
        )
        .route(
            "/api/admin/config/models/{model_id}/default",
            put(admin_config::set_default_model),
        )
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

fn seed_admin(state: &AppState) -> anyhow::Result<()> {
    if state.user_repo.count()? > 0 {
        return Ok(());
    }

    let password_hash = auth_service::hash_password(&state.config.auth.admin_password)
        .context("Failed to hash admin password")?;

    state.user_repo.create(
        &state.config.auth.admin_username,
        &state.config.auth.admin_email,
        &password_hash,
        &UserRole::Admin,
    )?;

    tracing::info!(
        "Admin account seeded: {} ({})",
        state.config.auth.admin_username,
        state.config.auth.admin_email
    );

    Ok(())
}
