use anyhow::{Context, Result};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use serde::Serialize;
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
    pool: Pool<SqliteConnectionManager>,
}

impl InviteRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    pub fn create(
        &self,
        email: &str,
        role: &UserRole,
        invited_by: &str,
        expires_hours: i64,
    ) -> Result<Invite> {
        let conn = self.pool.get().context("Failed to get db connection")?;
        let id = Uuid::new_v4().to_string();
        let token = Uuid::new_v4().to_string();
        let now = chrono::Utc::now();
        let expires_at = now
            .checked_add_signed(chrono::Duration::hours(expires_hours))
            .unwrap_or(now)
            .to_rfc3339();
        let created_at = now.to_rfc3339();

        conn.execute(
            "INSERT INTO user_invites (id, email, token, role, invited_by, used, expires_at, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?7)",
            params![id, email, token, role.to_string(), invited_by, expires_at, created_at],
        )
        .context("Failed to create invite")?;

        Ok(Invite {
            id,
            email: email.to_string(),
            token,
            role: role.clone(),
            invited_by: invited_by.to_string(),
            used: false,
            expires_at,
            created_at,
        })
    }

    pub fn find_by_token(&self, token: &str) -> Result<Option<Invite>> {
        let conn = self.pool.get().context("Failed to get db connection")?;
        let mut stmt = conn
            .prepare(
                "SELECT id, email, token, role, invited_by, used, expires_at, created_at
                 FROM user_invites WHERE token = ?1",
            )
            .context("Failed to prepare query")?;

        let invite = stmt
            .query_row(params![token], |row| {
                Ok(Invite {
                    id: row.get(0)?,
                    email: row.get(1)?,
                    token: row.get(2)?,
                    role: UserRole::try_from(row.get::<_, String>(3)?.as_str()).unwrap(),
                    invited_by: row.get(4)?,
                    used: row.get::<_, i32>(5)? != 0,
                    expires_at: row.get(6)?,
                    created_at: row.get(7)?,
                })
            })
            .optional();

        match invite {
            Ok(i) => Ok(i),
            Err(e) => Err(anyhow::anyhow!("Query error: {e}")),
        }
    }

    pub fn mark_used(&self, token: &str) -> Result<()> {
        let conn = self.pool.get().context("Failed to get db connection")?;
        conn.execute(
            "UPDATE user_invites SET used = 1 WHERE token = ?1",
            params![token],
        )
        .context("Failed to mark invite as used")?;
        Ok(())
    }

    pub fn find_all(&self) -> Result<Vec<Invite>> {
        let conn = self.pool.get().context("Failed to get db connection")?;
        let mut stmt = conn
            .prepare(
                "SELECT id, email, token, role, invited_by, used, expires_at, created_at
                 FROM user_invites ORDER BY created_at DESC",
            )
            .context("Failed to prepare query")?;

        let invites = stmt
            .query_map([], |row| {
                Ok(Invite {
                    id: row.get(0)?,
                    email: row.get(1)?,
                    token: row.get(2)?,
                    role: UserRole::try_from(row.get::<_, String>(3)?.as_str()).unwrap(),
                    invited_by: row.get(4)?,
                    used: row.get::<_, i32>(5)? != 0,
                    expires_at: row.get(6)?,
                    created_at: row.get(7)?,
                })
            })
            .context("Failed to query invites")?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to collect invites")?;

        Ok(invites)
    }
}

trait OptionalRow {
    fn optional(self) -> Result<Option<Invite>, rusqlite::Error>;
}

impl OptionalRow for std::result::Result<Invite, rusqlite::Error> {
    fn optional(self) -> Result<Option<Invite>, rusqlite::Error> {
        match self {
            Ok(invite) => Ok(Some(invite)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}
