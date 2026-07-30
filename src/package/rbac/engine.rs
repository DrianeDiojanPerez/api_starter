use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use uuid::Uuid;

use crate::package::rbac::{split_action, Engine, Store};

pub struct RbacEngine {
    super_role: String,
    store: Arc<dyn Store>,
}

impl RbacEngine {
    pub fn new(super_role: impl Into<String>, store: Arc<dyn Store>) -> Self {
        Self {
            super_role: super_role.into(),
            store,
        }
    }

    async fn role_bypass(&self, user_id: Uuid) -> bool {
        match self.store.get_roles(user_id).await {
            Ok(roles) => roles.contains(&self.super_role),
            Err(error) => {
                tracing::debug!(%error, "failed to retrieve user roles");
                false
            }
        }
    }
}

#[async_trait]
impl Engine for RbacEngine {
    async fn can(&self, user_id: Uuid, action: &str) -> bool {
        tracing::debug!(action, "attempting to check if user is allowed");

        if self.role_bypass(user_id).await {
            tracing::debug!("user contains super role, skipping checking");
            return true;
        }

        let Some((resource, permission)) = split_action(action) else {
            tracing::debug!(action, "permission to check is not valid");
            return false;
        };

        match self
            .store
            .has_permission(user_id, resource, permission)
            .await
        {
            Ok(allowed) => allowed,
            Err(error) => {
                tracing::debug!(%error, "failed to verify user permission");
                false
            }
        }
    }

    async fn can_any(&self, user_id: Uuid, actions: &[&str]) -> bool {
        if self.role_bypass(user_id).await {
            tracing::debug!("user contains super role, skipping checking");
            return true;
        }

        let permissions = match self.store.get_permissions(user_id).await {
            Ok(permissions) => permissions,
            Err(error) => {
                tracing::debug!(%error, "failed to retrieve users permissions");
                return false;
            }
        };

        let mut granted: HashMap<&str, Vec<&str>> = HashMap::new();
        for permission in &permissions {
            granted
                .entry(permission.resource.as_str())
                .or_default()
                .push(permission.name.as_str());
        }

        actions.iter().any(|action| {
            split_action(action).is_some_and(|(resource, permission)| {
                granted
                    .get(resource)
                    .is_some_and(|names| names.contains(&permission))
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::sdk::Permission;

    struct FakeStore {
        roles: Vec<String>,
        permissions: Vec<Permission>,
    }

    #[async_trait]
    impl Store for FakeStore {
        async fn get_roles(&self, _user_id: Uuid) -> Result<Vec<String>, sqlx::Error> {
            Ok(self.roles.clone())
        }

        async fn get_permissions(&self, _user_id: Uuid) -> Result<Vec<Permission>, sqlx::Error> {
            Ok(self.permissions.clone())
        }

        async fn has_permission(
            &self,
            _user_id: Uuid,
            resource: &str,
            permission: &str,
        ) -> Result<bool, sqlx::Error> {
            Ok(self
                .permissions
                .iter()
                .any(|p| p.resource == resource && p.name == permission))
        }
    }

    fn engine(roles: &[&str], permissions: &[(&str, &str)]) -> RbacEngine {
        RbacEngine::new(
            "Admin",
            Arc::new(FakeStore {
                roles: roles.iter().map(|role| (*role).to_owned()).collect(),
                permissions: permissions
                    .iter()
                    .enumerate()
                    .map(|(index, (resource, name))| Permission {
                        id: index as i32,
                        name: (*name).to_owned(),
                        resource: (*resource).to_owned(),
                        module_id: 1,
                    })
                    .collect(),
            }),
        )
    }

    #[tokio::test]
    async fn the_super_role_bypasses_permission_checks() {
        let engine = engine(&["Admin"], &[]);

        assert!(engine.can(Uuid::new_v4(), "Users.View All").await);
    }

    #[tokio::test]
    async fn grants_an_explicitly_assigned_permission() {
        let engine = engine(&["Staff"], &[("Users", "View All")]);

        assert!(engine.can(Uuid::new_v4(), "Users.View All").await);
        assert!(!engine.can(Uuid::new_v4(), "Users.Delete").await);
    }

    #[tokio::test]
    async fn rejects_a_malformed_action() {
        let engine = engine(&["Staff"], &[("Users", "View All")]);

        assert!(!engine.can(Uuid::new_v4(), "Users").await);
    }

    #[tokio::test]
    async fn can_any_matches_a_single_granted_action() {
        let engine = engine(&["Staff"], &[("Users", "View All")]);
        let user_id = Uuid::new_v4();

        assert!(
            engine
                .can_any(user_id, &["Users.Delete", "Users.View All"])
                .await
        );
        assert!(!engine.can_any(user_id, &["Users.Delete"]).await);
    }
}
