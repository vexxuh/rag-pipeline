use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
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
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    Admin,
    Maintainer,
    User,
}

impl UserRole {
    pub fn level(&self) -> u8 {
        match self {
            UserRole::User => 0,
            UserRole::Maintainer => 1,
            UserRole::Admin => 2,
        }
    }

    pub fn is_at_least(&self, required: &UserRole) -> bool {
        self.level() >= required.level()
    }
}

impl std::fmt::Display for UserRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UserRole::Admin => write!(f, "admin"),
            UserRole::Maintainer => write!(f, "maintainer"),
            UserRole::User => write!(f, "user"),
        }
    }
}

impl TryFrom<&str> for UserRole {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "admin" => Ok(UserRole::Admin),
            "maintainer" => Ok(UserRole::Maintainer),
            "user" => Ok(UserRole::User),
            other => Err(anyhow::anyhow!("Invalid role: {other}")),
        }
    }
}

#[derive(Clone)]
pub struct UserRepository {
    pool: PgPool,
}

impl UserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        username: &str,
        email: &str,
        password_hash: &str,
        role: &UserRole,
    ) -> Result<User> {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now();

        sqlx::query(
            "INSERT INTO users (id, username, email, password_hash, role, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(&id)
        .bind(username)
        .bind(email)
        .bind(password_hash)
        .bind(role.to_string())
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .context("Failed to insert user")?;

        Ok(User {
            id,
            username: username.to_string(),
            email: email.to_string(),
            password_hash: password_hash.to_string(),
            role: role.clone(),
            created_at: now.to_rfc3339(),
            updated_at: now.to_rfc3339(),
        })
    }

    pub async fn find_by_email(&self, email: &str) -> Result<Option<User>> {
        let row = sqlx::query(
            "SELECT id, username, email, password_hash, role,
                    to_char(created_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at,
                    to_char(updated_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS updated_at
             FROM users WHERE email = $1",
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to query user by email")?;

        row.map(|r| map_row(&r)).transpose()
    }

    pub async fn find_by_id(&self, id: &str) -> Result<Option<User>> {
        let row = sqlx::query(
            "SELECT id, username, email, password_hash, role,
                    to_char(created_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at,
                    to_char(updated_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS updated_at
             FROM users WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to query user by id")?;

        row.map(|r| map_row(&r)).transpose()
    }

    pub async fn find_all(&self) -> Result<Vec<User>> {
        let rows = sqlx::query(
            "SELECT id, username, email, password_hash, role,
                    to_char(created_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at,
                    to_char(updated_at, 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS updated_at
             FROM users ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to query users")?;

        rows.iter().map(|r| map_row(r)).collect()
    }

    pub async fn update_role(&self, id: &str, role: &UserRole) -> Result<()> {
        let now = chrono::Utc::now();

        sqlx::query("UPDATE users SET role = $1, updated_at = $2 WHERE id = $3")
            .bind(role.to_string())
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await
            .context("Failed to update user role")?;

        Ok(())
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .context("Failed to delete user")?;

        Ok(())
    }

    pub async fn count(&self) -> Result<i64> {
        let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users")
            .fetch_one(&self.pool)
            .await
            .context("Failed to count users")?;

        Ok(count)
    }
}

fn map_row(row: &sqlx::postgres::PgRow) -> Result<User> {
    let role_str: String = row.try_get("role").context("Failed to get role")?;
    let role = UserRole::try_from(role_str.as_str())?;

    Ok(User {
        id: row.try_get("id").context("Failed to get id")?,
        username: row.try_get("username").context("Failed to get username")?,
        email: row.try_get("email").context("Failed to get email")?,
        password_hash: row
            .try_get("password_hash")
            .context("Failed to get password_hash")?,
        role,
        created_at: row
            .try_get("created_at")
            .context("Failed to get created_at")?,
        updated_at: row
            .try_get("updated_at")
            .context("Failed to get updated_at")?,
    })
}
