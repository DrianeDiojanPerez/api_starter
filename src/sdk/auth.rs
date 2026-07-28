use chrono::{DateTime, Utc};

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
