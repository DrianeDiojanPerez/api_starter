use serde::Serialize;

use crate::sdk::AuthenticationTokens;

/// Token pair returned by login and refresh.
#[derive(Debug, Clone, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub refresh_token: String,
}

impl From<AuthenticationTokens> for LoginResponse {
    fn from(tokens: AuthenticationTokens) -> Self {
        Self {
            token: tokens.token,
            refresh_token: tokens.refresh_token,
        }
    }
}
