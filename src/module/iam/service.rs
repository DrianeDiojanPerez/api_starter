use std::sync::Arc;

use crate::database::{Database, TxManager};
use crate::module::iam::adapter::repository::{PgPermissionRepository, PgUserRepository};
use crate::module::iam::core::ports::{PermissionService, UserService};
use crate::module::iam::core::service::{PermissionServiceImpl, UserServiceImpl};

/// Wires the repositories and services of the module together, mirroring the
/// `NewService` constructor of the Go implementation.
#[derive(Clone)]
pub struct Services {
    pub user: Arc<dyn UserService>,
    pub permission: Arc<dyn PermissionService>,
}

impl Services {
    pub fn new(db: Arc<Database>, trm: TxManager) -> Self {
        let user_repository = Arc::new(PgUserRepository::new(db.clone()));
        let permission_repository = Arc::new(PgPermissionRepository::new(db));

        Self {
            user: Arc::new(UserServiceImpl::new(trm, user_repository)),
            permission: Arc::new(PermissionServiceImpl::new(permission_repository)),
        }
    }
}
