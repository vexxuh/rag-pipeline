use anyhow::{Context, Result};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlJob {
    pub id: String,
    pub user_id: String,
    pub url: String,
    pub crawl_type: String,
    pub status: String,
    pub pages_found: i64,
    pub pages_processed: i64,
    pub error_message: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

#[derive(Clone)]
pub struct CrawlJobRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl CrawlJobRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    pub fn create(&self, user_id: &str, url: &str, crawl_type: &str) -> Result<CrawlJob> {
        let conn = self.pool.get().context("Failed to get db connection")?;
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO crawl_jobs (id, user_id, url, crawl_type, status, created_at)
             VALUES (?1, ?2, ?3, ?4, 'pending', ?5)",
            params![id, user_id, url, crawl_type, now],
        )
        .context("Failed to insert crawl job")?;

        Ok(CrawlJob {
            id,
            user_id: user_id.to_string(),
            url: url.to_string(),
            crawl_type: crawl_type.to_string(),
            status: "pending".to_string(),
            pages_found: 0,
            pages_processed: 0,
            error_message: None,
            created_at: now,
            started_at: None,
            completed_at: None,
        })
    }

    pub fn find_by_id(&self, id: &str) -> Result<Option<CrawlJob>> {
        let conn = self.pool.get().context("Failed to get db connection")?;
        let mut stmt = conn.prepare(
            "SELECT id, user_id, url, crawl_type, status, pages_found, pages_processed,
                    error_message, created_at, started_at, completed_at
             FROM crawl_jobs WHERE id = ?1",
        )?;

        let job = stmt
            .query_row(params![id], Self::map_row)
            .optional()
            .context("Failed to query crawl job")?;

        Ok(job)
    }

    pub fn find_by_user(&self, user_id: &str) -> Result<Vec<CrawlJob>> {
        let conn = self.pool.get().context("Failed to get db connection")?;
        let mut stmt = conn.prepare(
            "SELECT id, user_id, url, crawl_type, status, pages_found, pages_processed,
                    error_message, created_at, started_at, completed_at
             FROM crawl_jobs WHERE user_id = ?1 ORDER BY created_at DESC",
        )?;

        let jobs = stmt
            .query_map(params![user_id], Self::map_row)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to collect crawl jobs")?;

        Ok(jobs)
    }

    pub fn update_status(
        &self,
        id: &str,
        status: &str,
        pages_found: Option<i64>,
        pages_processed: Option<i64>,
        error_message: Option<&str>,
    ) -> Result<()> {
        let conn = self.pool.get().context("Failed to get db connection")?;
        let now = chrono::Utc::now().to_rfc3339();

        let started_at = if status == "running" {
            Some(now.clone())
        } else {
            None
        };
        let completed_at = if status == "completed" || status == "failed" {
            Some(now)
        } else {
            None
        };

        conn.execute(
            "UPDATE crawl_jobs SET
                status = ?1,
                pages_found = COALESCE(?2, pages_found),
                pages_processed = COALESCE(?3, pages_processed),
                error_message = ?4,
                started_at = COALESCE(?5, started_at),
                completed_at = COALESCE(?6, completed_at)
             WHERE id = ?7",
            params![
                status,
                pages_found,
                pages_processed,
                error_message,
                started_at,
                completed_at,
                id
            ],
        )
        .context("Failed to update crawl job")?;

        Ok(())
    }

    fn map_row(row: &rusqlite::Row) -> rusqlite::Result<CrawlJob> {
        Ok(CrawlJob {
            id: row.get(0)?,
            user_id: row.get(1)?,
            url: row.get(2)?,
            crawl_type: row.get(3)?,
            status: row.get(4)?,
            pages_found: row.get(5)?,
            pages_processed: row.get(6)?,
            error_message: row.get(7)?,
            created_at: row.get(8)?,
            started_at: row.get(9)?,
            completed_at: row.get(10)?,
        })
    }
}

trait OptionalRow<T> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error>;
}

impl<T> OptionalRow<T> for std::result::Result<T, rusqlite::Error> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error> {
        match self {
            Ok(val) => Ok(Some(val)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}
