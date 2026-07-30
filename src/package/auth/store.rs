use async_trait::async_trait;
use sqlx::postgres::PgRow;
use sqlx::Row;
use uuid::Uuid;

use crate::database::Database;
use crate::package::auth::{Identity, PasswordReset};

#[async_trait]
pub trait Store: Send + Sync {
    async fn find_user_by_id(&self, user_id: Uuid) -> Result<Option<Identity>, sqlx::Error>;
    async fn find_user_by_email(&self, email: &str) -> Result<Option<Identity>, sqlx::Error>;
    async fn create_password_reset(&self, email: &str, token: &str) -> Result<(), sqlx::Error>;
    async fn reset_password(&self, email: &str, new_password: &str) -> Result<(), sqlx::Error>;
    async fn find_password_by_token(
        &self,
        token: &str,
    ) -> Result<Option<PasswordReset>, sqlx::Error>;
    async fn delete_password_reset(&self, email: &str) -> Result<(), sqlx::Error>;
}

pub struct PostgresAuthStore {
    db: std::sync::Arc<Database>,
}

impl PostgresAuthStore {
    pub fn new(db: std::sync::Arc<Database>) -> Self {
        Self { db }
    }

    async fn get_user_roles(&self, user_id: Uuid) -> Result<Vec<String>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT r.name \
             FROM iam.roles r \
             INNER JOIN iam.user_has_roles uhr ON r.id = uhr.role_id \
             WHERE uhr.user_id = $1",
        )
        .bind(user_id)
        .fetch_all(self.db.pool())
        .await?;

        rows.into_iter().map(|row| row.try_get("name")).collect()
    }

    async fn enrich(&self, row: Option<PgRow>) -> Result<Option<Identity>, sqlx::Error> {
        let Some(row) = row else {
            return Ok(None);
        };

        let id: Uuid = row.try_get("id")?;

        Ok(Some(Identity {
            id,
            email: row.try_get("email")?,
            user_name: row.try_get("user_name")?,
            password: row.try_get("password")?,
            roles: self.get_user_roles(id).await?,
        }))
    }
}

const SELECT_USER: &str = "SELECT u.id, u.email, u.user_name, u.password FROM iam.users u";

#[async_trait]
impl Store for PostgresAuthStore {
    async fn find_user_by_id(&self, user_id: Uuid) -> Result<Option<Identity>, sqlx::Error> {
        let row = sqlx::query(&format!("{SELECT_USER} WHERE u.id = $1"))
            .bind(user_id)
            .fetch_optional(self.db.pool())
            .await?;

        self.enrich(row).await
    }

    async fn find_user_by_email(&self, email: &str) -> Result<Option<Identity>, sqlx::Error> {
        let row = sqlx::query(&format!("{SELECT_USER} WHERE u.email = $1"))
            .bind(email)
            .fetch_optional(self.db.pool())
            .await?;

        self.enrich(row).await
    }

    async fn create_password_reset(&self, email: &str, token: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO iam.password_resets (email, token, created_at) VALUES ($1, $2, NOW())",
        )
        .bind(email)
        .bind(token)
        .execute(self.db.pool())
        .await?;

        Ok(())
    }

    async fn reset_password(&self, email: &str, new_password: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE iam.users SET password = $1 WHERE email = $2")
            .bind(new_password)
            .bind(email)
            .execute(self.db.pool())
            .await?;

        Ok(())
    }

    async fn find_password_by_token(
        &self,
        token: &str,
    ) -> Result<Option<PasswordReset>, sqlx::Error> {
        let row = sqlx::query(
            "SELECT email, token, created_at FROM iam.password_resets WHERE token = $1",
        )
        .bind(token)
        .fetch_optional(self.db.pool())
        .await?;

        row.map(|row| {
            Ok(PasswordReset {
                email: row.try_get("email")?,
                token: row.try_get("token")?,
                created_at: row.try_get("created_at")?,
            })
        })
        .transpose()
    }

    async fn delete_password_reset(&self, email: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM iam.password_resets WHERE email = $1")
            .bind(email)
            .execute(self.db.pool())
            .await?;

        Ok(())
    }
}
