mod engine;
mod store;

pub use engine::RbacEngine;
pub use store::{PostgresRbacStore, Store};

use async_trait::async_trait;
use uuid::Uuid;

/// Permission checks used by the authorization middleware. Actions are written
/// as `Resource.Permission`, for example `Users.View All`.
#[async_trait]
pub trait Engine: Send + Sync {
    async fn can(&self, user_id: Uuid, action: &str) -> bool;
    async fn can_any(&self, user_id: Uuid, actions: &[&str]) -> bool;
}

/// Splits `Users.View All` into `("Users", "View All")`.
pub fn split_action(action: &str) -> Option<(&str, &str)> {
    action.split_once('.')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_on_the_first_separator() {
        assert_eq!(split_action("Users.View All"), Some(("Users", "View All")));
        assert_eq!(
            split_action("Catalogues.Add.Tags"),
            Some(("Catalogues", "Add.Tags"))
        );
        assert_eq!(split_action("Users"), None);
    }
}
