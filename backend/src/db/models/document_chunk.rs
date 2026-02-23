use anyhow::{Context, Result};
use serde::Serialize;
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct DocumentChunk {
    pub id: String,
    pub source_type: String,
    pub source_id: String,
    pub chunk_index: i32,
    pub content: String,
    pub qdrant_point_id: String,
    pub created_at: String,
}

#[derive(Clone)]
pub struct DocumentChunkRepository {
    pool: PgPool,
}

impl DocumentChunkRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_batch(
        &self,
        chunks: &[(String, String, i32, String, String)], // (source_type, source_id, chunk_index, content, qdrant_point_id)
    ) -> Result<()> {
        for (source_type, source_id, chunk_index, content, qdrant_point_id) in chunks {
            let id = Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO document_chunks (id, source_type, source_id, chunk_index, content, qdrant_point_id)
                 VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(&id)
            .bind(source_type)
            .bind(source_id)
            .bind(chunk_index)
            .bind(content)
            .bind(qdrant_point_id)
            .execute(&self.pool)
            .await
            .context("Failed to insert document chunk")?;
        }

        Ok(())
    }

    pub async fn find_by_source(&self, source_type: &str, source_id: &str) -> Result<Vec<DocumentChunk>> {
        let rows = sqlx::query(
            "SELECT id, source_type, source_id, chunk_index, content, qdrant_point_id,
                    to_char(created_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at
             FROM document_chunks WHERE source_type = $1 AND source_id = $2
             ORDER BY chunk_index ASC",
        )
        .bind(source_type)
        .bind(source_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to find chunks by source")?;

        let chunks = rows
            .iter()
            .map(|row| DocumentChunk {
                id: row.get("id"),
                source_type: row.get("source_type"),
                source_id: row.get("source_id"),
                chunk_index: row.get("chunk_index"),
                content: row.get("content"),
                qdrant_point_id: row.get("qdrant_point_id"),
                created_at: row.get("created_at"),
            })
            .collect();

        Ok(chunks)
    }

    pub async fn delete_by_source(&self, source_type: &str, source_id: &str) -> Result<Vec<String>> {
        let rows = sqlx::query(
            "DELETE FROM document_chunks WHERE source_type = $1 AND source_id = $2 RETURNING qdrant_point_id",
        )
        .bind(source_type)
        .bind(source_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to delete chunks by source")?;

        let point_ids: Vec<String> = rows.iter().map(|row| row.get("qdrant_point_id")).collect();
        Ok(point_ids)
    }

    pub async fn find_by_qdrant_ids(&self, point_ids: &[String]) -> Result<Vec<DocumentChunk>> {
        if point_ids.is_empty() {
            return Ok(Vec::new());
        }

        let rows = sqlx::query(
            "SELECT id, source_type, source_id, chunk_index, content, qdrant_point_id,
                    to_char(created_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at
             FROM document_chunks WHERE qdrant_point_id = ANY($1)",
        )
        .bind(point_ids)
        .fetch_all(&self.pool)
        .await
        .context("Failed to find chunks by qdrant ids")?;

        let chunks = rows
            .iter()
            .map(|row| DocumentChunk {
                id: row.get("id"),
                source_type: row.get("source_type"),
                source_id: row.get("source_id"),
                chunk_index: row.get("chunk_index"),
                content: row.get("content"),
                qdrant_point_id: row.get("qdrant_point_id"),
                created_at: row.get("created_at"),
            })
            .collect();

        Ok(chunks)
    }
}
