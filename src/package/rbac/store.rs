use std::sync::Arc;

use async_trait::async_trait;
use sqlx::Row;
use uuid::Uuid;

use crate::database::Database;
use crate::package::rbac::Permission;

#[async_trait]
pub trait Store: Send + Sync {
    async fn get_roles(&self, user_id: Uuid) -> Result<Vec<String>, sqlx::Error>;
    async fn get_permissions(&self, user_id: Uuid) -> Result<Vec<Permission>, sqlx::Error>;
    async fn has_permission(
        &self,
        user_id: Uuid,
        resource: &str,
        permission: &str,
    ) -> Result<bool, sqlx::Error>;
}

pub struct PostgresRbacStore {
    db: Arc<Database>,
}

impl PostgresRbacStore {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Store for PostgresRbacStore {
    async fn get_roles(&self, user_id: Uuid) -> Result<Vec<String>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT r.name FROM iam.roles r \
             WHERE r.id IN (SELECT uhr.role_id FROM iam.user_has_roles uhr WHERE uhr.user_id = $1)",
        )
        .bind(user_id)
        .fetch_all(self.db.pool())
        .await?;

        rows.into_iter().map(|row| row.try_get("name")).collect()
    }

    async fn get_permissions(&self, user_id: Uuid) -> Result<Vec<Permission>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT p.resource, p.name \
             FROM iam.permissions p \
             INNER JOIN iam.role_has_permissions rhp ON p.id = rhp.permission_id \
             INNER JOIN iam.user_has_roles uhr ON uhr.role_id = rhp.role_id \
             WHERE uhr.user_id = $1",
        )
        .bind(user_id)
        .fetch_all(self.db.pool())
        .await?;

        rows.into_iter()
            .map(|row| {
                Ok(Permission {
                    resource: row.try_get("resource")?,
                    name: row.try_get("name")?,
                })
            })
            .collect()
    }

    async fn has_permission(
        &self,
        user_id: Uuid,
        resource: &str,
        permission: &str,
    ) -> Result<bool, sqlx::Error> {
        let row = sqlx::query(
            "SELECT EXISTS ( \
                 SELECT 1 \
                 FROM iam.permissions p \
                 INNER JOIN iam.role_has_permissions rhp ON p.id = rhp.permission_id \
                 INNER JOIN iam.user_has_roles uhr ON uhr.role_id = rhp.role_id \
                 WHERE uhr.user_id = $1 AND p.resource = $2 AND p.name = $3 \
             ) AS has_permission",
        )
        .bind(user_id)
        .bind(resource)
        .bind(permission)
        .fetch_one(self.db.pool())
        .await?;

        row.try_get("has_permission")
    }
}
