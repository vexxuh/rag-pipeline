use crate::config::AppConfig;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub db: Pool<SqliteConnectionManager>,
}

impl AppState {
    pub fn new(config: AppConfig, db: Pool<SqliteConnectionManager>) -> Self {
        Self {
            config: Arc::new(config),
            db,
        }
    }
}
