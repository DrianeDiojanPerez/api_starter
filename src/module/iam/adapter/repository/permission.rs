use std::sync::Arc;

use async_trait::async_trait;
use sqlx::postgres::PgRow;
use sqlx::Row;

use crate::database::Database;
use crate::module::iam::core::domain::{DomainError, Permission};
use crate::module::iam::core::ports::PermissionRepository;

const SELECT_PERMISSION: &str = "SELECT p.id, p.name, p.resource, m.name AS module \
     FROM iam.permissions p \
     INNER JOIN iam.modules m ON p.module_id = m.id";

pub struct PgPermissionRepository {
    db: Arc<Database>,
}

impl PgPermissionRepository {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    fn map_permission(row: &PgRow) -> Result<Permission, sqlx::Error> {
        Ok(Permission {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            resource: row.try_get("resource")?,
            module: row.try_get("module")?,
        })
    }
}

#[async_trait]
impl PermissionRepository for PgPermissionRepository {
    async fn list_all(&self) -> Result<Vec<Permission>, DomainError> {
        let rows = sqlx::query(SELECT_PERMISSION)
            .fetch_all(self.db.pool())
            .await?;

        Ok(rows
            .iter()
            .map(Self::map_permission)
            .collect::<Result<Vec<_>, _>>()?)
    }

    async fn list_permissions_by_role(&self, role_id: i32) -> Result<Vec<Permission>, DomainError> {
        let rows = sqlx::query(&format!(
            "{SELECT_PERMISSION} \
             INNER JOIN iam.role_has_permissions rp ON p.id = rp.permission_id \
             WHERE rp.role_id = $1"
        ))
        .bind(role_id)
        .fetch_all(self.db.pool())
        .await?;

        Ok(rows
            .iter()
            .map(Self::map_permission)
            .collect::<Result<Vec<_>, _>>()?)
    }
}
