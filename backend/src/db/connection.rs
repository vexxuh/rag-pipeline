use crate::config::DatabaseConfig;
use anyhow::{Context, Result};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use std::path::Path;

pub fn create_pool(config: &DatabaseConfig) -> Result<Pool<SqliteConnectionManager>> {
    let db_path = Path::new(&config.path);
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).context("Failed to create database directory")?;
    }

    let manager = SqliteConnectionManager::file(&config.path);
    let pool = Pool::builder()
        .max_size(config.max_connections)
        .build(manager)
        .context("Failed to create database pool")?;

    // Enable WAL mode for better concurrent read performance
    let conn = pool.get().context("Failed to get connection from pool")?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
        .context("Failed to set SQLite pragmas")?;

    Ok(pool)
}
