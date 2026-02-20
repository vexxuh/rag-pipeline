use anyhow::Context;
use axum::{
    middleware as axum_mw,
    routing::{delete, get, post, put},
    Router,
};
use axum::http::HeaderName;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use rag_backend::config::AppConfig;
use rag_backend::db::models::user::UserRole;
use rag_backend::db::{connection, migrations};
use rag_backend::middleware::auth::auth_middleware;
use rag_backend::middleware::embed_auth::embed_auth_middleware;
use rag_backend::routes::{admin, admin_audit, admin_config, admin_embed, admin_logs, auth, chat, crawl, documents, health, settings, widget};
use rag_backend::services::auth_service;
use rag_backend::services::storage::StorageService;
use rag_backend::services::vector::VectorService;
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

    let db_pool = connection::create_pool(&config.database)
        .await
        .context("Failed to create database pool")?;

    migrations::run_all(&db_pool)
        .await
        .context("Failed to run migrations")?;

    let storage = StorageService::new(&config.minio)
        .await
        .context("Failed to initialize MinIO storage")?;
    tracing::info!("MinIO storage initialized");

    let vector_service = VectorService::new(&config.qdrant)
        .await
        .context("Failed to initialize Qdrant vector service")?;
    tracing::info!("Qdrant vector service initialized");

    let state = AppState::new(config.clone(), db_pool, storage, vector_service);

    // Seed admin account on first boot
    seed_admin(&state).await?;

    // Seed widget system user for anonymous widget conversations
    seed_widget_user(&state).await?;

    // Seed default provider/model catalogue
    state
        .admin_config_repo
        .seed_defaults()
        .await
        .context("Failed to seed admin config defaults")?;

    // Spawn background task to purge soft-deleted conversations older than 30 days
    {
        let conversation_repo = state.conversation_repo.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(24 * 60 * 60));
            loop {
                interval.tick().await;
                match conversation_repo.hard_delete_expired().await {
                    Ok(count) if count > 0 => {
                        tracing::info!("Purged {count} expired soft-deleted conversations");
                    }
                    Ok(_) => {}
                    Err(e) => {
                        tracing::error!("Failed to purge expired conversations: {e}");
                    }
                }
            }
        });
    }

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
            HeaderName::from_static("x-embed-key"),
            HeaderName::from_static("x-session-id"),
        ]);

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
        .route("/api/documents/rescan", post(documents::rescan))
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
        // Admin — Logs
        .route("/api/admin/logs", get(admin_logs::list_conversation_logs))
        .route(
            "/api/admin/logs/{id}",
            get(admin_logs::get_conversation_log),
        )
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
        // Admin — Audit logs
        .route(
            "/api/admin/audit-logs",
            get(admin_audit::list_audit_logs),
        )
        // Admin — Embed keys
        .route("/api/admin/embed-keys", post(admin_embed::create_key))
        .route("/api/admin/embed-keys", get(admin_embed::list_keys))
        .route(
            "/api/admin/embed-keys/{id}",
            get(admin_embed::get_key),
        )
        .route(
            "/api/admin/embed-keys/{id}",
            put(admin_embed::update_key),
        )
        .route(
            "/api/admin/embed-keys/{id}",
            delete(admin_embed::delete_key),
        )
        .route(
            "/api/admin/embed-keys/{id}/toggle",
            put(admin_embed::toggle_key),
        )
        .layer(axum_mw::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    let widget_routes = Router::new()
        .route("/api/widget/config", get(widget::get_config))
        .route(
            "/api/widget/conversations",
            post(widget::create_conversation),
        )
        .route(
            "/api/widget/conversations",
            get(widget::list_conversations),
        )
        .route(
            "/api/widget/conversations/{id}/messages",
            get(widget::get_messages),
        )
        .route(
            "/api/widget/conversations/{id}/messages",
            post(widget::send_message),
        )
        .layer(axum_mw::from_fn_with_state(
            state.clone(),
            embed_auth_middleware,
        ));

    let app = Router::new()
        .merge(public_routes)
        .merge(widget_routes)
        .merge(protected_routes)
        .nest_service("/static", ServeDir::new("static"))
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

async fn seed_widget_user(state: &AppState) -> anyhow::Result<()> {
    // Create a system user for widget conversations (satisfies FK constraint)
    sqlx::query(
        "INSERT INTO users (id, username, email, password_hash, role)
         VALUES ('__widget__', 'widget', 'widget@system.internal', '__no_login__', 'user')
         ON CONFLICT (id) DO NOTHING"
    )
    .execute(&state.db)
    .await
    .context("Failed to seed widget user")?;

    Ok(())
}

async fn seed_admin(state: &AppState) -> anyhow::Result<()> {
    if state.user_repo.count().await? > 0 {
        return Ok(());
    }

    let password_hash = auth_service::hash_password(&state.config.auth.admin_password)
        .context("Failed to hash admin password")?;

    state
        .user_repo
        .create(
            &state.config.auth.admin_username,
            &state.config.auth.admin_email,
            &password_hash,
            &UserRole::Admin,
        )
        .await?;

    tracing::info!(
        "Admin account seeded: {} ({})",
        state.config.auth.admin_username,
        state.config.auth.admin_email
    );

    Ok(())
}
