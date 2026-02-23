use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
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
    pool: PgPool,
}

impl CrawlJobRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, user_id: &str, url: &str, crawl_type: &str) -> Result<CrawlJob> {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now();

        sqlx::query(
            "INSERT INTO crawl_jobs (id, user_id, url, crawl_type, status, created_at)
             VALUES ($1, $2, $3, $4, 'pending', $5)",
        )
        .bind(&id)
        .bind(user_id)
        .bind(url)
        .bind(crawl_type)
        .bind(now)
        .execute(&self.pool)
        .await
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
            created_at: now.to_rfc3339(),
            started_at: None,
            completed_at: None,
        })
    }

    pub async fn find_by_id(&self, id: &str) -> Result<Option<CrawlJob>> {
        let row = sqlx::query(
            "SELECT id, user_id, url, crawl_type, status, pages_found, pages_processed,
                    error_message,
                    to_char(created_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at,
                    to_char(started_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS started_at,
                    to_char(completed_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS completed_at
             FROM crawl_jobs WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to query crawl job")?;

        Ok(row.map(Self::map_row))
    }

    pub async fn find_by_user(&self, user_id: &str) -> Result<Vec<CrawlJob>> {
        let rows = sqlx::query(
            "SELECT id, user_id, url, crawl_type, status, pages_found, pages_processed,
                    error_message,
                    to_char(created_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at,
                    to_char(started_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS started_at,
                    to_char(completed_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS completed_at
             FROM crawl_jobs WHERE user_id = $1 ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to list crawl jobs")?;

        Ok(rows.into_iter().map(Self::map_row).collect())
    }

    pub async fn update_status(
        &self,
        id: &str,
        status: &str,
        pages_found: Option<i64>,
        pages_processed: Option<i64>,
        error_message: Option<&str>,
    ) -> Result<()> {
        let now = chrono::Utc::now();

        let started_at: Option<chrono::DateTime<chrono::Utc>> = if status == "running" {
            Some(now)
        } else {
            None
        };
        let completed_at: Option<chrono::DateTime<chrono::Utc>> =
            if status == "completed" || status == "failed" {
                Some(now)
            } else {
                None
            };

        sqlx::query(
            "UPDATE crawl_jobs SET
                status = $1,
                pages_found = COALESCE($2, pages_found),
                pages_processed = COALESCE($3, pages_processed),
                error_message = $4,
                started_at = COALESCE($5, started_at),
                completed_at = COALESCE($6, completed_at)
             WHERE id = $7",
        )
        .bind(status)
        .bind(pages_found)
        .bind(pages_processed)
        .bind(error_message)
        .bind(started_at)
        .bind(completed_at)
        .bind(id)
        .execute(&self.pool)
        .await
        .context("Failed to update crawl job")?;

        Ok(())
    }

    fn map_row(row: sqlx::postgres::PgRow) -> CrawlJob {
        CrawlJob {
            id: row.get("id"),
            user_id: row.get("user_id"),
            url: row.get("url"),
            crawl_type: row.get("crawl_type"),
            status: row.get("status"),
            pages_found: row.get("pages_found"),
            pages_processed: row.get("pages_processed"),
            error_message: row.get("error_message"),
            created_at: row.get("created_at"),
            started_at: row.get("started_at"),
            completed_at: row.get("completed_at"),
        }
    }
}
