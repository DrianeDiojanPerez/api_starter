mod domain;
mod service;
mod store;

pub use domain::{AuthenticationTokens, Identity, PasswordReset};
pub use service::AuthService;
pub use store::{PostgresAuthStore, Store};

use async_trait::async_trait;
use uuid::Uuid;

use crate::package::errdef::Error;

#[async_trait]
pub trait Auth: Send + Sync {
    async fn generate_token(
        &self,
        email: &str,
        password: &str,
    ) -> Result<AuthenticationTokens, Error>;

    async fn refresh_token(&self, refresh_token: &str) -> Result<AuthenticationTokens, Error>;

    async fn get_identity(&self, access_token: &str) -> Result<Identity, Error>;

    async fn password_recovery(&self, email: &str, callback_uri: &str) -> Result<(), Error>;

    async fn reset_password(&self, token: &str, new_password: &str) -> Result<(), Error>;
}

#[derive(Debug, Clone)]
pub struct AuthUser(pub Identity);

impl AuthUser {
    pub fn id(&self) -> Uuid {
        self.0.id
    }
}
