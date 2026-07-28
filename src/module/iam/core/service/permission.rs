use std::sync::Arc;

use async_trait::async_trait;

use crate::module::iam::core::domain::Permission;
use crate::module::iam::core::ports::{PermissionRepository, PermissionService};
use crate::shared::errdef::Error;

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
