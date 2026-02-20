use anyhow::{Context, Result};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub role: UserRole,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    Admin,
    User,
}

impl std::fmt::Display for UserRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UserRole::Admin => write!(f, "admin"),
            UserRole::User => write!(f, "user"),
        }
    }
}

impl TryFrom<&str> for UserRole {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "admin" => Ok(UserRole::Admin),
            "user" => Ok(UserRole::User),
            other => Err(anyhow::anyhow!("Invalid role: {other}")),
        }
    }
}

#[derive(Clone)]
pub struct UserRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl UserRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    pub fn create(
        &self,
        username: &str,
        email: &str,
        password_hash: &str,
        role: &UserRole,
    ) -> Result<User> {
        let conn = self.pool.get().context("Failed to get db connection")?;
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO users (id, username, email, password_hash, role, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, username, email, password_hash, role.to_string(), now, now],
        )
        .context("Failed to insert user")?;

        Ok(User {
            id,
            username: username.to_string(),
            email: email.to_string(),
            password_hash: password_hash.to_string(),
            role: role.clone(),
            created_at: now.clone(),
            updated_at: now,
        })
    }

    pub fn find_by_email(&self, email: &str) -> Result<Option<User>> {
        let conn = self.pool.get().context("Failed to get db connection")?;
        let mut stmt = conn
            .prepare(
                "SELECT id, username, email, password_hash, role, created_at, updated_at
                 FROM users WHERE email = ?1",
            )
            .context("Failed to prepare query")?;

        let user = stmt
            .query_row(params![email], |row| {
                Ok(User {
                    id: row.get(0)?,
                    username: row.get(1)?,
                    email: row.get(2)?,
                    password_hash: row.get(3)?,
                    role: UserRole::try_from(row.get::<_, String>(4)?.as_str()).unwrap(),
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })
            .optional();

        match user {
            Ok(u) => Ok(u),
            Err(e) => Err(anyhow::anyhow!("Query error: {e}")),
        }
    }

    pub fn find_by_id(&self, id: &str) -> Result<Option<User>> {
        let conn = self.pool.get().context("Failed to get db connection")?;
        let mut stmt = conn
            .prepare(
                "SELECT id, username, email, password_hash, role, created_at, updated_at
                 FROM users WHERE id = ?1",
            )
            .context("Failed to prepare query")?;

        let user = stmt
            .query_row(params![id], |row| {
                Ok(User {
                    id: row.get(0)?,
                    username: row.get(1)?,
                    email: row.get(2)?,
                    password_hash: row.get(3)?,
                    role: UserRole::try_from(row.get::<_, String>(4)?.as_str()).unwrap(),
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })
            .optional();

        match user {
            Ok(u) => Ok(u),
            Err(e) => Err(anyhow::anyhow!("Query error: {e}")),
        }
    }

    pub fn find_all(&self) -> Result<Vec<User>> {
        let conn = self.pool.get().context("Failed to get db connection")?;
        let mut stmt = conn
            .prepare(
                "SELECT id, username, email, password_hash, role, created_at, updated_at
                 FROM users ORDER BY created_at DESC",
            )
            .context("Failed to prepare query")?;

        let users = stmt
            .query_map([], |row| {
                Ok(User {
                    id: row.get(0)?,
                    username: row.get(1)?,
                    email: row.get(2)?,
                    password_hash: row.get(3)?,
                    role: UserRole::try_from(row.get::<_, String>(4)?.as_str()).unwrap(),
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })
            .context("Failed to query users")?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to collect users")?;

        Ok(users)
    }

    pub fn update_role(&self, id: &str, role: &UserRole) -> Result<()> {
        let conn = self.pool.get().context("Failed to get db connection")?;
        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE users SET role = ?1, updated_at = ?2 WHERE id = ?3",
            params![role.to_string(), now, id],
        )
        .context("Failed to update user role")?;

        Ok(())
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        let conn = self.pool.get().context("Failed to get db connection")?;
        conn.execute("DELETE FROM users WHERE id = ?1", params![id])
            .context("Failed to delete user")?;
        Ok(())
    }

    pub fn count(&self) -> Result<i64> {
        let conn = self.pool.get().context("Failed to get db connection")?;
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))
            .context("Failed to count users")?;
        Ok(count)
    }
}

trait OptionalRow {
    fn optional(self) -> Result<Option<User>, rusqlite::Error>;
}

impl OptionalRow for std::result::Result<User, rusqlite::Error> {
    fn optional(self) -> Result<Option<User>, rusqlite::Error> {
        match self {
            Ok(user) => Ok(Some(user)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}
