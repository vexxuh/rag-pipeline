use anyhow::{Context, Result};
use serde::Serialize;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use super::user::UserRole;

#[derive(Debug, Clone, Serialize)]
pub struct Invite {
    pub id: String,
    pub email: String,
    pub token: String,
    pub role: UserRole,
    pub invited_by: String,
    pub used: bool,
    pub expires_at: String,
    pub created_at: String,
}

#[derive(Clone)]
pub struct InviteRepository {
    pool: PgPool,
}

impl InviteRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        email: &str,
        role: &UserRole,
        invited_by: &str,
        expires_hours: i64,
    ) -> Result<Invite> {
        let id = Uuid::new_v4().to_string();
        let token = Uuid::new_v4().to_string();
        let now = chrono::Utc::now();
        let expires_at = now
            .checked_add_signed(chrono::Duration::hours(expires_hours))
            .unwrap_or(now);
        let created_at = now;

        sqlx::query(
            "INSERT INTO user_invites (id, email, token, role, invited_by, used, expires_at, created_at)
             VALUES ($1, $2, $3, $4, $5, false, $6, $7)",
        )
        .bind(&id)
        .bind(email)
        .bind(&token)
        .bind(role.to_string())
        .bind(invited_by)
        .bind(expires_at)
        .bind(created_at)
        .execute(&self.pool)
        .await
        .context("Failed to create invite")?;

        Ok(Invite {
            id,
            email: email.to_string(),
            token,
            role: role.clone(),
            invited_by: invited_by.to_string(),
            used: false,
            expires_at: expires_at.to_rfc3339(),
            created_at: created_at.to_rfc3339(),
        })
    }

    pub async fn find_by_token(&self, token: &str) -> Result<Option<Invite>> {
        let row = sqlx::query(
            "SELECT id, email, token, role, invited_by, used,
                    to_char(expires_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS expires_at,
                    to_char(created_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at
             FROM user_invites WHERE token = $1",
        )
        .bind(token)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to query invite by token")?;

        let invite = row
            .map(|r| -> Result<Invite> {
                let role_str: String = r.get("role");
                Ok(Invite {
                    id: r.get("id"),
                    email: r.get("email"),
                    token: r.get("token"),
                    role: UserRole::try_from(role_str.as_str())
                        .map_err(|e| anyhow::anyhow!("Invalid role: {e}"))?,
                    invited_by: r.get("invited_by"),
                    used: r.get::<bool, _>("used"),
                    expires_at: r.get("expires_at"),
                    created_at: r.get("created_at"),
                })
            })
            .transpose()?;

        Ok(invite)
    }

    pub async fn mark_used(&self, token: &str) -> Result<()> {
        sqlx::query("UPDATE user_invites SET used = TRUE WHERE token = $1")
            .bind(token)
            .execute(&self.pool)
            .await
            .context("Failed to mark invite as used")?;
        Ok(())
    }

    pub async fn find_all(&self) -> Result<Vec<Invite>> {
        let rows = sqlx::query(
            "SELECT id, email, token, role, invited_by, used,
                    to_char(expires_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS expires_at,
                    to_char(created_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at
             FROM user_invites ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to query invites")?;

        let invites = rows
            .into_iter()
            .map(|r| {
                let role_str: String = r.get("role");
                Ok(Invite {
                    id: r.get("id"),
                    email: r.get("email"),
                    token: r.get("token"),
                    role: UserRole::try_from(role_str.as_str())
                        .map_err(|e| anyhow::anyhow!("Invalid role: {e}"))?,
                    invited_by: r.get("invited_by"),
                    used: r.get::<bool, _>("used"),
                    expires_at: r.get("expires_at"),
                    created_at: r.get("created_at"),
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(invites)
    }
}
