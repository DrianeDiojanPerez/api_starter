use std::sync::Arc;

use async_trait::async_trait;

use crate::module::iam::core::domain::Permission;
use crate::module::iam::core::ports::{PermissionRepository, PermissionService};
use crate::package::errdef::Error;

pub struct PermissionServiceImpl {
    repository: Arc<dyn PermissionRepository>,
}

impl PermissionServiceImpl {
    pub fn new(repository: Arc<dyn PermissionRepository>) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl PermissionService for PermissionServiceImpl {
    async fn list_all(&self) -> Result<Vec<Permission>, Error> {
        tracing::debug!("Retrieving all Permissions");

        self.repository.list_all().await.map_err(Error::unknown)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use async_trait::async_trait;

    use crate::module::iam::core::domain::DomainError;

    struct FakePermissionRepository {
        permissions: Result<Vec<Permission>, fn() -> DomainError>,
    }

    #[async_trait]
    impl PermissionRepository for FakePermissionRepository {
        async fn list_all(&self) -> Result<Vec<Permission>, DomainError> {
            match &self.permissions {
                Ok(permissions) => Ok(permissions.clone()),
                Err(error) => Err(error()),
            }
        }

        async fn list_permissions_by_role(
            &self,
            _role_id: i32,
        ) -> Result<Vec<Permission>, DomainError> {
            unimplemented!("not reachable from the current routes")
        }
    }

    #[tokio::test]
    async fn lists_every_permission() {
        let service = PermissionServiceImpl::new(Arc::new(FakePermissionRepository {
            permissions: Ok(vec![Permission {
                id: 1,
                name: "View All".to_owned(),
                resource: "Users".to_owned(),
                module: "IAM Module".to_owned(),
            }]),
        }));

        let permissions = service.list_all().await.expect("listing should succeed");

        assert_eq!(permissions.len(), 1);
        assert_eq!(permissions[0].resource, "Users");
    }

    #[tokio::test]
    async fn a_database_failure_stays_internal() {
        let service = PermissionServiceImpl::new(Arc::new(FakePermissionRepository {
            permissions: Err(|| DomainError::Database(sqlx::Error::RowNotFound)),
        }));

        let err = service.list_all().await.expect_err("listing should fail");

        assert!(err.to_string().contains("internal server error"));
    }
}
