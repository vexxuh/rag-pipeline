use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub user_id: String,
    pub filename: String,
    pub original_filename: String,
    pub minio_key: String,
    pub content_type: String,
    pub size_bytes: i64,
    pub status: DocumentStatus,
    pub error_message: Option<String>,
    pub created_at: String,
    pub processed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum DocumentStatus {
    Uploading,
    Processing,
    Ready,
    Failed,
}

impl std::fmt::Display for DocumentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DocumentStatus::Uploading => write!(f, "uploading"),
            DocumentStatus::Processing => write!(f, "processing"),
            DocumentStatus::Ready => write!(f, "ready"),
            DocumentStatus::Failed => write!(f, "failed"),
        }
    }
}

impl TryFrom<&str> for DocumentStatus {
    type Error = anyhow::Error;
    fn try_from(value: &str) -> Result<Self> {
        match value {
            "uploading" => Ok(DocumentStatus::Uploading),
            "processing" => Ok(DocumentStatus::Processing),
            "ready" => Ok(DocumentStatus::Ready),
            "failed" => Ok(DocumentStatus::Failed),
            other => Err(anyhow::anyhow!("Invalid document status: {other}")),
        }
    }
}

#[derive(Clone)]
pub struct DocumentRepository {
    pool: PgPool,
}

impl DocumentRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        user_id: &str,
        original_filename: &str,
        minio_key: &str,
        content_type: &str,
        size_bytes: i64,
    ) -> Result<Document> {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now();

        sqlx::query(
            "INSERT INTO documents
                 (id, user_id, filename, original_filename, minio_key, content_type, size_bytes, status, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        )
        .bind(&id)
        .bind(user_id)
        .bind(&id) // filename = id placeholder
        .bind(original_filename)
        .bind(minio_key)
        .bind(content_type)
        .bind(size_bytes)
        .bind("uploading")
        .bind(now)
        .execute(&self.pool)
        .await
        .context("Failed to insert document")?;

        Ok(Document {
            id: id.clone(),
            user_id: user_id.to_string(),
            filename: id,
            original_filename: original_filename.to_string(),
            minio_key: minio_key.to_string(),
            content_type: content_type.to_string(),
            size_bytes,
            status: DocumentStatus::Uploading,
            error_message: None,
            created_at: now.to_rfc3339(),
            processed_at: None,
        })
    }

    pub async fn find_by_id(&self, id: &str) -> Result<Option<Document>> {
        let row = sqlx::query(
            "SELECT id, user_id, filename, original_filename, minio_key, content_type,
                    size_bytes, status, error_message,
                    to_char(created_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at,
                    to_char(processed_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS processed_at
             FROM documents WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to query document")?;

        row.map(|r| Self::map_row(&r)).transpose()
    }

    pub async fn find_by_user(&self, user_id: &str) -> Result<Vec<Document>> {
        let rows = sqlx::query(
            "SELECT id, user_id, filename, original_filename, minio_key, content_type,
                    size_bytes, status, error_message,
                    to_char(created_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at,
                    to_char(processed_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS processed_at
             FROM documents WHERE user_id = $1 ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to list documents")?;

        rows.iter().map(|r| Self::map_row(r)).collect()
    }

    pub async fn update_minio_key(&self, id: &str, minio_key: &str) -> Result<()> {
        sqlx::query("UPDATE documents SET minio_key = $1 WHERE id = $2")
            .bind(minio_key)
            .bind(id)
            .execute(&self.pool)
            .await
            .context("Failed to update document minio_key")?;
        Ok(())
    }

    pub async fn update_status(
        &self,
        id: &str,
        status: &DocumentStatus,
        error_message: Option<&str>,
    ) -> Result<()> {
        let now = chrono::Utc::now();
        let processed_at: Option<chrono::DateTime<chrono::Utc>> =
            if *status == DocumentStatus::Ready || *status == DocumentStatus::Failed {
                Some(now)
            } else {
                None
            };

        sqlx::query(
            "UPDATE documents SET status = $1, error_message = $2, processed_at = $3 WHERE id = $4",
        )
        .bind(status.to_string())
        .bind(error_message)
        .bind(processed_at)
        .bind(id)
        .execute(&self.pool)
        .await
        .context("Failed to update document status")?;

        Ok(())
    }

    pub async fn find_all_ready(&self) -> Result<Vec<Document>> {
        let rows = sqlx::query(
            "SELECT id, user_id, filename, original_filename, minio_key, content_type,
                    size_bytes, status, error_message,
                    to_char(created_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at,
                    to_char(processed_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS processed_at
             FROM documents WHERE status = 'ready' ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to list ready documents")?;

        rows.iter().map(|r| Self::map_row(r)).collect()
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM documents WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .context("Failed to delete document")?;

        Ok(())
    }

    fn map_row(row: &sqlx::postgres::PgRow) -> Result<Document> {
        let status_str: String = row.try_get("status").context("Failed to get status")?;
        let status = DocumentStatus::try_from(status_str.as_str())?;

        Ok(Document {
            id: row.try_get("id").context("Failed to get id")?,
            user_id: row.try_get("user_id").context("Failed to get user_id")?,
            filename: row.try_get("filename").context("Failed to get filename")?,
            original_filename: row
                .try_get("original_filename")
                .context("Failed to get original_filename")?,
            minio_key: row.try_get("minio_key").context("Failed to get minio_key")?,
            content_type: row
                .try_get("content_type")
                .context("Failed to get content_type")?,
            size_bytes: row.try_get("size_bytes").context("Failed to get size_bytes")?,
            status,
            error_message: row.try_get("error_message").context("Failed to get error_message")?,
            created_at: row.try_get("created_at").context("Failed to get created_at")?,
            processed_at: row
                .try_get("processed_at")
                .context("Failed to get processed_at")?,
        })
    }
}
