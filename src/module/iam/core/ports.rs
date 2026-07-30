use async_trait::async_trait;
use serde::Deserialize;
use sqlx::PgConnection;
use uuid::Uuid;

use crate::module::iam::core::domain::{CreateUser, DomainError, Permission, User};
use crate::package::errdef::Error;
use crate::package::pagination::{Data, ListRequest};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateUser {
    pub avatar_id: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: Option<String>,
    pub department_id: Option<i32>,
    pub status: Option<String>,
    pub password: Option<String>,
    pub add_roles: Option<Vec<String>>,
    pub remove_roles: Option<Vec<String>>,
}

impl UpdateUser {
    pub fn has_no_column_changes(&self) -> bool {
        self.avatar_id.is_none()
            && self.first_name.is_none()
            && self.last_name.is_none()
            && self.email.is_none()
            && self.department_id.is_none()
            && self.status.is_none()
            && self.password.is_none()
    }

    pub fn restricted_to_self(self) -> Self {
        Self {
            avatar_id: self.avatar_id,
            first_name: self.first_name,
            last_name: self.last_name,
            department_id: self.department_id,
            ..Self::default()
        }
    }
}

#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn index(&self, request: &ListRequest) -> Result<(Vec<User>, i64), DomainError>;
    async fn create(&self, tx: &mut PgConnection, user: &CreateUser) -> Result<Uuid, DomainError>;
    async fn find_by_id(&self, user_id: Uuid) -> Result<Option<User>, DomainError>;
    async fn find_by_user_name(&self, user_name: &str) -> Result<Option<User>, DomainError>;
    async fn find_by_email(&self, email: &str) -> Result<Option<User>, DomainError>;
    async fn add_user_roles(
        &self,
        tx: &mut PgConnection,
        user_id: Uuid,
        roles: &[String],
    ) -> Result<(), DomainError>;
    async fn remove_user_roles(
        &self,
        tx: &mut PgConnection,
        user_id: Uuid,
        roles: &[String],
    ) -> Result<(), DomainError>;
    async fn partial_update(
        &self,
        tx: &mut PgConnection,
        user_id: Uuid,
        fields: &UpdateUser,
    ) -> Result<(), DomainError>;
    async fn delete(&self, tx: &mut PgConnection, user_id: Uuid) -> Result<(), DomainError>;
}

#[async_trait]
pub trait PermissionRepository: Send + Sync {
    async fn list_all(&self) -> Result<Vec<Permission>, DomainError>;
    async fn list_permissions_by_role(&self, role_id: i32) -> Result<Vec<Permission>, DomainError>;
}

#[async_trait]
pub trait UserService: Send + Sync {
    async fn index(&self, request: ListRequest) -> Result<Data<User>, Error>;
    async fn create(&self, new_user: CreateUser) -> Result<Uuid, Error>;
    async fn find_by_id(&self, user_id: Uuid) -> Result<User, Error>;
    async fn partial_update(&self, user_id: Uuid, fields: UpdateUser) -> Result<(), Error>;
    async fn delete(&self, user_id: Uuid) -> Result<(), Error>;
}

#[async_trait]
pub trait PermissionService: Send + Sync {
    async fn list_all(&self) -> Result<Vec<Permission>, Error>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restricting_to_self_drops_privileged_fields() {
        let payload = UpdateUser {
            first_name: Some("Ada".to_owned()),
            status: Some("Deleted".to_owned()),
            password: Some("hunter2".to_owned()),
            add_roles: Some(vec!["Admin".to_owned()]),
            ..UpdateUser::default()
        }
        .restricted_to_self();

        assert_eq!(payload.first_name.as_deref(), Some("Ada"));
        assert!(payload.status.is_none());
        assert!(payload.password.is_none());
        assert!(payload.add_roles.is_none());
    }

    #[test]
    fn detects_a_role_only_payload() {
        let payload = UpdateUser {
            add_roles: Some(vec!["Staff".to_owned()]),
            ..UpdateUser::default()
        };

        assert!(payload.has_no_column_changes());
    }
}
