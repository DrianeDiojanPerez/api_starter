use chrono::{DateTime, Utc};
use uuid::Uuid;

/// The password hash never leaves the auth layer, so it is skipped during
/// serialization.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Identity {
    pub id: Uuid,
    pub email: String,
    pub user_name: String,
    #[serde(skip)]
    pub password: String,
    pub roles: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AuthenticationTokens {
    pub token: String,
    pub refresh_token: String,
}

#[derive(Debug, Clone)]
pub struct PasswordReset {
    pub email: String,
    pub token: String,
    pub created_at: DateTime<Utc>,
}
