use anyhow::{Context, Result};
use serde::Serialize;
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct AuditLog {
    pub id: String,
    pub user_id: Option<String>,
    pub event_type: String,
    pub resource_type: Option<String>,
    pub resource_id: Option<String>,
    pub description: String,
    pub ip_address: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: String,
}

#[derive(Clone)]
pub struct AuditLogRepository {
    pool: PgPool,
}

impl AuditLogRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        user_id: Option<&str>,
        event_type: &str,
        resource_type: Option<&str>,
        resource_id: Option<&str>,
        description: &str,
        ip_address: Option<&str>,
        metadata: Option<serde_json::Value>,
    ) -> Result<()> {
        let id = Uuid::new_v4().to_string();
        let meta = metadata.unwrap_or(serde_json::json!({}));

        sqlx::query(
            "INSERT INTO audit_logs (id, user_id, event_type, resource_type, resource_id, description, ip_address, metadata)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(&id)
        .bind(user_id)
        .bind(event_type)
        .bind(resource_type)
        .bind(resource_id)
        .bind(description)
        .bind(ip_address)
        .bind(&meta)
        .execute(&self.pool)
        .await
        .context("Failed to create audit log")?;

        Ok(())
    }

    pub async fn list(
        &self,
        user_id: Option<&str>,
        event_type: Option<&str>,
        from: Option<&str>,
        to: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AuditLog>> {
        let mut query = String::from(
            "SELECT id, user_id, event_type, resource_type, resource_id, description,
                    ip_address, metadata,
                    to_char(created_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at
             FROM audit_logs WHERE 1=1",
        );
        let mut param_idx = 1u32;
        let mut binds: Vec<String> = Vec::new();

        if let Some(uid) = user_id {
            query.push_str(&format!(" AND user_id = ${param_idx}"));
            param_idx += 1;
            binds.push(uid.to_string());
        }
        if let Some(et) = event_type {
            query.push_str(&format!(" AND event_type = ${param_idx}"));
            param_idx += 1;
            binds.push(et.to_string());
        }
        if let Some(f) = from {
            query.push_str(&format!(" AND created_at >= ${param_idx}::timestamptz"));
            param_idx += 1;
            binds.push(f.to_string());
        }
        if let Some(t) = to {
            query.push_str(&format!(" AND created_at <= ${param_idx}::timestamptz"));
            param_idx += 1;
            binds.push(t.to_string());
        }

        query.push_str(&format!(" ORDER BY created_at DESC LIMIT ${param_idx}"));
        param_idx += 1;
        query.push_str(&format!(" OFFSET ${param_idx}"));

        let mut q = sqlx::query(&query);
        for b in &binds {
            q = q.bind(b);
        }
        q = q.bind(limit).bind(offset);

        let rows = q.fetch_all(&self.pool).await.context("Failed to list audit logs")?;

        let logs = rows
            .iter()
            .map(|row| AuditLog {
                id: row.get("id"),
                user_id: row.get("user_id"),
                event_type: row.get("event_type"),
                resource_type: row.get("resource_type"),
                resource_id: row.get("resource_id"),
                description: row.get("description"),
                ip_address: row.get("ip_address"),
                metadata: row.get("metadata"),
                created_at: row.get("created_at"),
            })
            .collect();

        Ok(logs)
    }

    pub async fn count(
        &self,
        user_id: Option<&str>,
        event_type: Option<&str>,
        from: Option<&str>,
        to: Option<&str>,
    ) -> Result<i64> {
        let mut query = String::from("SELECT COUNT(*) FROM audit_logs WHERE 1=1");
        let mut param_idx = 1u32;
        let mut binds: Vec<String> = Vec::new();

        if let Some(uid) = user_id {
            query.push_str(&format!(" AND user_id = ${param_idx}"));
            param_idx += 1;
            binds.push(uid.to_string());
        }
        if let Some(et) = event_type {
            query.push_str(&format!(" AND event_type = ${param_idx}"));
            param_idx += 1;
            binds.push(et.to_string());
        }
        if let Some(f) = from {
            query.push_str(&format!(" AND created_at >= ${param_idx}::timestamptz"));
            param_idx += 1;
            binds.push(f.to_string());
        }
        if let Some(t) = to {
            query.push_str(&format!(" AND created_at <= ${param_idx}::timestamptz"));
            #[allow(unused_assignments)]
            { param_idx += 1; }
            binds.push(t.to_string());
        }

        let mut q = sqlx::query_scalar::<_, i64>(&query);
        for b in &binds {
            q = q.bind(b);
        }

        let count = q.fetch_one(&self.pool).await.context("Failed to count audit logs")?;
        Ok(count)
    }
}
