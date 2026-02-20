use anyhow::{Context, Result};
use sqlx::PgPool;

pub async fn run_all(pool: &PgPool) -> Result<()> {
    create_users_table(pool).await?;
    create_user_invites_table(pool).await?;
    add_maintainer_role(pool).await?;
    create_user_api_keys_table(pool).await?;
    create_user_llm_preferences_table(pool).await?;
    create_admin_config_tables(pool).await?;
    create_conversation_tables(pool).await?;
    create_documents_table(pool).await?;
    create_crawl_tables(pool).await?;
    create_audit_logs_table(pool).await?;
    add_soft_delete_to_conversations(pool).await?;
    create_document_chunks_table(pool).await?;
    create_embed_keys_table(pool).await?;
    create_widget_sessions_table(pool).await?;
    add_widget_columns_to_conversations(pool).await?;
    tracing::info!("Database migrations completed");
    Ok(())
}

async fn create_users_table(pool: &PgPool) -> Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY,
            username TEXT NOT NULL UNIQUE,
            email TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL,
            role TEXT NOT NULL CHECK(role IN ('admin', 'maintainer', 'user')) DEFAULT 'user',
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )",
    )
    .execute(pool)
    .await
    .context("Failed to create users table")?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_users_email ON users(email)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_users_username ON users(username)")
        .execute(pool)
        .await?;

    Ok(())
}

async fn create_user_invites_table(pool: &PgPool) -> Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS user_invites (
            id TEXT PRIMARY KEY,
            email TEXT NOT NULL,
            token TEXT NOT NULL UNIQUE,
            role TEXT NOT NULL CHECK(role IN ('admin', 'maintainer', 'user')) DEFAULT 'user',
            invited_by TEXT NOT NULL REFERENCES users(id),
            used BOOLEAN NOT NULL DEFAULT FALSE,
            expires_at TIMESTAMPTZ NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )",
    )
    .execute(pool)
    .await
    .context("Failed to create user_invites table")?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_invites_token ON user_invites(token)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_invites_email ON user_invites(email)")
        .execute(pool)
        .await?;

    Ok(())
}

async fn add_maintainer_role(pool: &PgPool) -> Result<()> {
    // For existing databases: update CHECK constraints to include 'maintainer'
    sqlx::query(
        "DO $$
        BEGIN
            ALTER TABLE users DROP CONSTRAINT IF EXISTS users_role_check;
            ALTER TABLE users ADD CONSTRAINT users_role_check
                CHECK(role IN ('admin', 'maintainer', 'user'));
            ALTER TABLE user_invites DROP CONSTRAINT IF EXISTS user_invites_role_check;
            ALTER TABLE user_invites ADD CONSTRAINT user_invites_role_check
                CHECK(role IN ('admin', 'maintainer', 'user'));
        END $$;",
    )
    .execute(pool)
    .await
    .context("Failed to add maintainer role to constraints")?;

    Ok(())
}

async fn create_user_api_keys_table(pool: &PgPool) -> Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS user_api_keys (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            provider TEXT NOT NULL,
            api_key TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            UNIQUE(user_id, provider)
        )",
    )
    .execute(pool)
    .await
    .context("Failed to create user_api_keys table")?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_api_keys_user ON user_api_keys(user_id)")
        .execute(pool)
        .await?;

    Ok(())
}

async fn create_user_llm_preferences_table(pool: &PgPool) -> Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS user_llm_preferences (
            user_id TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
            preferred_provider TEXT NOT NULL,
            preferred_model TEXT NOT NULL,
            preferred_embedding_model TEXT NOT NULL,
            system_prompt TEXT NOT NULL DEFAULT ''
        )",
    )
    .execute(pool)
    .await
    .context("Failed to create user_llm_preferences table")?;

    Ok(())
}

async fn create_admin_config_tables(pool: &PgPool) -> Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS admin_providers (
            id TEXT PRIMARY KEY,
            provider_id TEXT NOT NULL UNIQUE,
            display_name TEXT NOT NULL,
            enabled BOOLEAN NOT NULL DEFAULT TRUE,
            supports_completion BOOLEAN NOT NULL DEFAULT TRUE,
            supports_embeddings BOOLEAN NOT NULL DEFAULT FALSE,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )",
    )
    .execute(pool)
    .await
    .context("Failed to create admin_providers table")?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS admin_models (
            id TEXT PRIMARY KEY,
            provider_id TEXT NOT NULL,
            model_id TEXT NOT NULL,
            display_name TEXT NOT NULL,
            model_type TEXT NOT NULL CHECK(model_type IN ('completion', 'embedding')),
            is_default BOOLEAN NOT NULL DEFAULT FALSE,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            UNIQUE(provider_id, model_id, model_type)
        )",
    )
    .execute(pool)
    .await
    .context("Failed to create admin_models table")?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_admin_models_provider ON admin_models(provider_id)")
        .execute(pool)
        .await?;

    Ok(())
}

async fn create_conversation_tables(pool: &PgPool) -> Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS conversations (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            title TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )",
    )
    .execute(pool)
    .await
    .context("Failed to create conversations table")?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_conversations_user ON conversations(user_id)")
        .execute(pool)
        .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS messages (
            id TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
            role TEXT NOT NULL CHECK(role IN ('user', 'assistant')),
            content TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )",
    )
    .execute(pool)
    .await
    .context("Failed to create messages table")?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_messages_conversation ON messages(conversation_id)")
        .execute(pool)
        .await?;

    Ok(())
}

async fn create_documents_table(pool: &PgPool) -> Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS documents (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            filename TEXT NOT NULL,
            original_filename TEXT NOT NULL,
            minio_key TEXT NOT NULL,
            content_type TEXT NOT NULL,
            size_bytes BIGINT NOT NULL,
            status TEXT NOT NULL CHECK(status IN ('uploading', 'processing', 'ready', 'failed')),
            error_message TEXT,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            processed_at TIMESTAMPTZ
        )",
    )
    .execute(pool)
    .await
    .context("Failed to create documents table")?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_documents_user ON documents(user_id)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_documents_status ON documents(status)")
        .execute(pool)
        .await?;

    Ok(())
}

async fn create_crawl_tables(pool: &PgPool) -> Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS crawl_jobs (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            url TEXT NOT NULL,
            crawl_type TEXT NOT NULL CHECK(crawl_type IN ('sitemap', 'full')),
            status TEXT NOT NULL CHECK(status IN ('pending', 'running', 'completed', 'failed')),
            pages_found BIGINT NOT NULL DEFAULT 0,
            pages_processed BIGINT NOT NULL DEFAULT 0,
            error_message TEXT,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            started_at TIMESTAMPTZ,
            completed_at TIMESTAMPTZ
        )",
    )
    .execute(pool)
    .await
    .context("Failed to create crawl_jobs table")?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_crawl_jobs_user ON crawl_jobs(user_id)")
        .execute(pool)
        .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS crawled_pages (
            id TEXT PRIMARY KEY,
            job_id TEXT NOT NULL REFERENCES crawl_jobs(id) ON DELETE CASCADE,
            url TEXT NOT NULL,
            title TEXT,
            content_length BIGINT NOT NULL DEFAULT 0,
            status TEXT NOT NULL CHECK(status IN ('pending', 'processed', 'failed')),
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )",
    )
    .execute(pool)
    .await
    .context("Failed to create crawled_pages table")?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_crawled_pages_job ON crawled_pages(job_id)")
        .execute(pool)
        .await?;

    Ok(())
}

async fn create_audit_logs_table(pool: &PgPool) -> Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS audit_logs (
            id TEXT PRIMARY KEY,
            user_id TEXT,
            event_type TEXT NOT NULL,
            resource_type TEXT,
            resource_id TEXT,
            description TEXT NOT NULL,
            ip_address TEXT,
            metadata JSONB DEFAULT '{}',
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )",
    )
    .execute(pool)
    .await
    .context("Failed to create audit_logs table")?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_audit_logs_user ON audit_logs(user_id)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_audit_logs_event ON audit_logs(event_type)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_audit_logs_created ON audit_logs(created_at DESC)")
        .execute(pool)
        .await?;

    Ok(())
}

async fn add_soft_delete_to_conversations(pool: &PgPool) -> Result<()> {
    sqlx::query(
        "ALTER TABLE conversations ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ DEFAULT NULL",
    )
    .execute(pool)
    .await
    .context("Failed to add deleted_at to conversations")?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_conversations_deleted ON conversations(deleted_at) WHERE deleted_at IS NOT NULL",
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn create_document_chunks_table(pool: &PgPool) -> Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS document_chunks (
            id TEXT PRIMARY KEY,
            source_type TEXT NOT NULL,
            source_id TEXT NOT NULL,
            chunk_index INTEGER NOT NULL,
            content TEXT NOT NULL,
            qdrant_point_id TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )",
    )
    .execute(pool)
    .await
    .context("Failed to create document_chunks table")?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_chunks_source ON document_chunks(source_type, source_id)")
        .execute(pool)
        .await?;

    Ok(())
}

async fn create_embed_keys_table(pool: &PgPool) -> Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS embed_keys (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            key_hash TEXT NOT NULL UNIQUE,
            key_prefix TEXT NOT NULL,
            allowed_domains TEXT[] NOT NULL DEFAULT '{}',
            system_prompt TEXT NOT NULL DEFAULT '',
            rate_limit INTEGER NOT NULL DEFAULT 20,
            widget_title TEXT NOT NULL DEFAULT 'Chat with us',
            primary_color TEXT NOT NULL DEFAULT '#2563eb',
            greeting_message TEXT NOT NULL DEFAULT 'Hello! How can I help you?',
            provider TEXT NOT NULL DEFAULT '',
            model TEXT NOT NULL DEFAULT '',
            api_key_encrypted TEXT NOT NULL DEFAULT '',
            total_conversations BIGINT NOT NULL DEFAULT 0,
            total_messages BIGINT NOT NULL DEFAULT 0,
            is_active BOOLEAN NOT NULL DEFAULT TRUE,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )",
    )
    .execute(pool)
    .await
    .context("Failed to create embed_keys table")?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_embed_keys_hash ON embed_keys(key_hash)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_embed_keys_active ON embed_keys(is_active) WHERE is_active = TRUE")
        .execute(pool)
        .await?;

    Ok(())
}

async fn create_widget_sessions_table(pool: &PgPool) -> Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS widget_sessions (
            id TEXT PRIMARY KEY,
            embed_key_id TEXT NOT NULL REFERENCES embed_keys(id) ON DELETE CASCADE,
            session_id TEXT NOT NULL,
            message_count INTEGER NOT NULL DEFAULT 0,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            last_message_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            UNIQUE(embed_key_id, session_id)
        )",
    )
    .execute(pool)
    .await
    .context("Failed to create widget_sessions table")?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_widget_sessions_lookup ON widget_sessions(embed_key_id, session_id)")
        .execute(pool)
        .await?;

    Ok(())
}

async fn add_widget_columns_to_conversations(pool: &PgPool) -> Result<()> {
    sqlx::query("ALTER TABLE conversations ADD COLUMN IF NOT EXISTS source TEXT NOT NULL DEFAULT 'app'")
        .execute(pool)
        .await
        .context("Failed to add source to conversations")?;

    sqlx::query("ALTER TABLE conversations ADD COLUMN IF NOT EXISTS embed_key_id TEXT DEFAULT NULL")
        .execute(pool)
        .await?;

    sqlx::query("ALTER TABLE conversations ADD COLUMN IF NOT EXISTS session_id TEXT DEFAULT NULL")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_conversations_source ON conversations(source)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_conversations_session ON conversations(session_id) WHERE session_id IS NOT NULL")
        .execute(pool)
        .await?;

    Ok(())
}
