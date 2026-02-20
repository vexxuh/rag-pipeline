use anyhow::{Context, Result};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use serde::{Deserialize, Serialize};
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
    pool: Pool<SqliteConnectionManager>,
}

impl DocumentRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    pub fn create(
        &self,
        user_id: &str,
        original_filename: &str,
        minio_key: &str,
        content_type: &str,
        size_bytes: i64,
    ) -> Result<Document> {
        let conn = self.pool.get().context("Failed to get db connection")?;
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO documents (id, user_id, filename, original_filename, minio_key, content_type, size_bytes, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![id, user_id, id, original_filename, minio_key, content_type, size_bytes, "uploading", now],
        )
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
            created_at: now,
            processed_at: None,
        })
    }

    pub fn find_by_id(&self, id: &str) -> Result<Option<Document>> {
        let conn = self.pool.get().context("Failed to get db connection")?;
        let mut stmt = conn.prepare(
            "SELECT id, user_id, filename, original_filename, minio_key, content_type, size_bytes, status, error_message, created_at, processed_at
             FROM documents WHERE id = ?1",
        )?;

        let doc = stmt
            .query_row(params![id], Self::map_row)
            .optional();

        match doc {
            Ok(d) => Ok(d),
            Err(e) => Err(anyhow::anyhow!("Query error: {e}")),
        }
    }

    pub fn find_by_user(&self, user_id: &str) -> Result<Vec<Document>> {
        let conn = self.pool.get().context("Failed to get db connection")?;
        let mut stmt = conn.prepare(
            "SELECT id, user_id, filename, original_filename, minio_key, content_type, size_bytes, status, error_message, created_at, processed_at
             FROM documents WHERE user_id = ?1 ORDER BY created_at DESC",
        )?;

        let docs = stmt
            .query_map(params![user_id], Self::map_row)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to collect documents")?;

        Ok(docs)
    }

    pub fn update_status(
        &self,
        id: &str,
        status: &DocumentStatus,
        error_message: Option<&str>,
    ) -> Result<()> {
        let conn = self.pool.get().context("Failed to get db connection")?;
        let now = chrono::Utc::now().to_rfc3339();

        let processed_at = if *status == DocumentStatus::Ready || *status == DocumentStatus::Failed
        {
            Some(now)
        } else {
            None
        };

        conn.execute(
            "UPDATE documents SET status = ?1, error_message = ?2, processed_at = ?3 WHERE id = ?4",
            params![status.to_string(), error_message, processed_at, id],
        )
        .context("Failed to update document status")?;

        Ok(())
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        let conn = self.pool.get().context("Failed to get db connection")?;
        conn.execute("DELETE FROM documents WHERE id = ?1", params![id])
            .context("Failed to delete document")?;
        Ok(())
    }

    fn map_row(row: &rusqlite::Row) -> rusqlite::Result<Document> {
        Ok(Document {
            id: row.get(0)?,
            user_id: row.get(1)?,
            filename: row.get(2)?,
            original_filename: row.get(3)?,
            minio_key: row.get(4)?,
            content_type: row.get(5)?,
            size_bytes: row.get(6)?,
            status: DocumentStatus::try_from(row.get::<_, String>(7)?.as_str()).unwrap(),
            error_message: row.get(8)?,
            created_at: row.get(9)?,
            processed_at: row.get(10)?,
        })
    }
}

trait OptionalRow {
    fn optional(self) -> Result<Option<Document>, rusqlite::Error>;
}

impl OptionalRow for std::result::Result<Document, rusqlite::Error> {
    fn optional(self) -> Result<Option<Document>, rusqlite::Error> {
        match self {
            Ok(doc) => Ok(Some(doc)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}
