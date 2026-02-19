use anyhow::{Context, Result};
use rusqlite::Connection;

pub fn run_all(conn: &Connection) -> Result<()> {
    create_users_table(conn)?;
    create_user_api_keys_table(conn)?;
    create_user_llm_preferences_table(conn)?;
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
            FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
        );",
    )
    .context("Failed to create user_llm_preferences table")?;
    Ok(())
}
