use anyhow::{Context, Result};
use rusqlite::Connection;

pub fn run_all(conn: &Connection) -> Result<()> {
    create_users_table(conn)?;
    create_user_invites_table(conn)?;
    create_user_api_keys_table(conn)?;
    create_user_llm_preferences_table(conn)?;
    create_admin_config_tables(conn)?;
    create_conversation_tables(conn)?;
    create_documents_table(conn)?;
    create_crawl_tables(conn)?;
    tracing::info!("Database migrations completed");
    Ok(())
}

fn create_users_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY,
            username TEXT NOT NULL UNIQUE,
            email TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL,
            role TEXT NOT NULL CHECK(role IN ('admin', 'user')) DEFAULT 'user',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);
        CREATE INDEX IF NOT EXISTS idx_users_username ON users(username);",
    )
    .context("Failed to create users table")?;
    Ok(())
}

fn create_user_invites_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS user_invites (
            id TEXT PRIMARY KEY,
            email TEXT NOT NULL,
            token TEXT NOT NULL UNIQUE,
            role TEXT NOT NULL CHECK(role IN ('admin', 'user')) DEFAULT 'user',
            invited_by TEXT NOT NULL,
            used INTEGER NOT NULL DEFAULT 0,
            expires_at TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY(invited_by) REFERENCES users(id)
        );
        CREATE INDEX IF NOT EXISTS idx_invites_token ON user_invites(token);
        CREATE INDEX IF NOT EXISTS idx_invites_email ON user_invites(email);",
    )
    .context("Failed to create user_invites table")?;
    Ok(())
}

fn create_user_api_keys_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS user_api_keys (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            provider TEXT NOT NULL,
            api_key TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE,
            UNIQUE(user_id, provider)
        );
        CREATE INDEX IF NOT EXISTS idx_api_keys_user ON user_api_keys(user_id);",
    )
    .context("Failed to create user_api_keys table")?;
    Ok(())
}

fn create_user_llm_preferences_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS user_llm_preferences (
            user_id TEXT PRIMARY KEY,
            preferred_provider TEXT NOT NULL,
            preferred_model TEXT NOT NULL,
            preferred_embedding_model TEXT NOT NULL,
            system_prompt TEXT NOT NULL DEFAULT '',
            FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
        );",
    )
    .context("Failed to create user_llm_preferences table")?;
    Ok(())
}

fn create_admin_config_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS admin_providers (
            id TEXT PRIMARY KEY,
            provider_id TEXT NOT NULL UNIQUE,
            display_name TEXT NOT NULL,
            enabled INTEGER NOT NULL DEFAULT 1,
            supports_completion INTEGER NOT NULL DEFAULT 1,
            supports_embeddings INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS admin_models (
            id TEXT PRIMARY KEY,
            provider_id TEXT NOT NULL,
            model_id TEXT NOT NULL,
            display_name TEXT NOT NULL,
            model_type TEXT NOT NULL CHECK(model_type IN ('completion', 'embedding')),
            is_default INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            UNIQUE(provider_id, model_id, model_type)
        );
        CREATE INDEX IF NOT EXISTS idx_admin_models_provider ON admin_models(provider_id);",
    )
    .context("Failed to create admin config tables")?;
    Ok(())
}

fn create_conversation_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS conversations (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            title TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_conversations_user ON conversations(user_id);

        CREATE TABLE IF NOT EXISTS messages (
            id TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL,
            role TEXT NOT NULL CHECK(role IN ('user', 'assistant')),
            content TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY(conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_messages_conversation ON messages(conversation_id);",
    )
    .context("Failed to create conversation tables")?;
    Ok(())
}

fn create_documents_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS documents (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            filename TEXT NOT NULL,
            original_filename TEXT NOT NULL,
            minio_key TEXT NOT NULL,
            content_type TEXT NOT NULL,
            size_bytes INTEGER NOT NULL,
            status TEXT NOT NULL CHECK(status IN ('uploading', 'processing', 'ready', 'failed')),
            error_message TEXT,
            created_at TEXT NOT NULL,
            processed_at TEXT,
            FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_documents_user ON documents(user_id);
        CREATE INDEX IF NOT EXISTS idx_documents_status ON documents(status);",
    )
    .context("Failed to create documents table")?;
    Ok(())
}

fn create_crawl_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS crawl_jobs (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            url TEXT NOT NULL,
            crawl_type TEXT NOT NULL CHECK(crawl_type IN ('sitemap', 'full')),
            status TEXT NOT NULL CHECK(status IN ('pending', 'running', 'completed', 'failed')),
            pages_found INTEGER NOT NULL DEFAULT 0,
            pages_processed INTEGER NOT NULL DEFAULT 0,
            error_message TEXT,
            created_at TEXT NOT NULL,
            started_at TEXT,
            completed_at TEXT,
            FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_crawl_jobs_user ON crawl_jobs(user_id);

        CREATE TABLE IF NOT EXISTS crawled_pages (
            id TEXT PRIMARY KEY,
            job_id TEXT NOT NULL,
            url TEXT NOT NULL,
            title TEXT,
            content_length INTEGER NOT NULL DEFAULT 0,
            status TEXT NOT NULL CHECK(status IN ('pending', 'processed', 'failed')),
            created_at TEXT NOT NULL,
            FOREIGN KEY(job_id) REFERENCES crawl_jobs(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_crawled_pages_job ON crawled_pages(job_id);",
    )
    .context("Failed to create crawl tables")?;
    Ok(())
}
