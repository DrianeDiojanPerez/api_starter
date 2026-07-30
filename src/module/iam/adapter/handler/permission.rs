use std::sync::Arc;

use axum::extract::State;
use axum::Json as AxumJson;
use serde::Serialize;

use crate::module::iam::core::domain;
use crate::module::iam::core::ports::PermissionService;
use crate::package::errdef::Error;
use crate::package::response::{self, Response};

#[derive(Debug, Clone, Serialize)]
pub struct Permission {
    pub name: String,
    pub resource: String,
    pub module: String,
}

impl From<domain::Permission> for Permission {
    fn from(permission: domain::Permission) -> Self {
        Self {
            name: permission.name,
            resource: permission.resource,
            module: permission.module,
        }
    }
}

pub type PermissionState = Arc<dyn PermissionService>;

#[tracing::instrument(name = "PermissionService.ListAll", skip_all)]
pub async fn index(
    State(service): State<PermissionState>,
) -> Result<AxumJson<Response<Vec<Permission>>>, Error> {
    let permissions = service
        .list_all()
        .await?
        .into_iter()
        .map(Permission::from)
        .collect();

    Ok(response::ok(permissions))
}
