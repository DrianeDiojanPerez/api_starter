use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{Duration, Utc};
use serde_json::Value;
use uuid::Uuid;

use crate::sdk::{AuthenticationTokens, User};
use crate::shared::auth::{Auth, Store};
use crate::shared::emailer::Emailer;
use crate::shared::errdef::Error;
use crate::shared::jwt::{Claims, TokenGenerator};
use crate::shared::utils;

/// A reset link is only usable for this long after it was requested.
const PASSWORD_RESET_TTL_MINUTES: i64 = 15;

const INVALID_CREDENTIALS: &str = "invalid username or password";
const INVALID_REFRESH_TOKEN: &str = "invalid or malformed refresh token";

pub struct AuthService {
    jwt: Arc<dyn TokenGenerator>,
    store: Arc<dyn Store>,
    mailer: Arc<dyn Emailer>,
    token_ttl: i64,
    refresh_token_ttl: i64,
}

impl AuthService {
    pub fn new(
        jwt: Arc<dyn TokenGenerator>,
        store: Arc<dyn Store>,
        mailer: Arc<dyn Emailer>,
        token_ttl: i64,
        refresh_token_ttl: i64,
    ) -> Self {
        Self {
            jwt,
            store,
            mailer,
            token_ttl,
            refresh_token_ttl,
        }
    }

    fn generate_tokens(&self, user: &User) -> Result<AuthenticationTokens, Error> {
        let now = Utc::now();

        let access_claims = Claims::from([
            ("user_id".to_owned(), Value::from(user.id.to_string())),
            ("roles".to_owned(), Value::from(user.roles.clone())),
        ]);

        let token = self
            .jwt
            .generate_token(
                access_claims,
                (now + Duration::seconds(self.token_ttl)).timestamp(),
            )
            .map_err(Error::unknown)?;

        let refresh_claims =
            Claims::from([("user_id".to_owned(), Value::from(user.id.to_string()))]);

        let refresh_token = self
            .jwt
            .generate_token(
                refresh_claims,
                (now + Duration::seconds(self.refresh_token_ttl)).timestamp(),
            )
            .map_err(Error::unknown)?;

        Ok(AuthenticationTokens {
            token,
            refresh_token,
        })
    }

    /// Reads and parses the `user_id` claim out of a validated token.
    fn user_id_from(&self, token: &str) -> Result<Uuid, Error> {
        let claims = self
            .jwt
            .validate_token(token)
            .map_err(|err| Error::unauthorized(INVALID_REFRESH_TOKEN).with_cause(err))?;

        claims
            .get("user_id")
            .and_then(Value::as_str)
            .and_then(|raw| Uuid::parse_str(raw).ok())
            .ok_or_else(|| Error::unauthorized(INVALID_REFRESH_TOKEN))
    }

    async fn require_user_by_id(&self, user_id: Uuid) -> Result<User, Error> {
        self.store
            .find_user_by_id(user_id)
            .await
            .map_err(Error::unknown)?
            .ok_or_else(|| Error::unauthorized(INVALID_REFRESH_TOKEN))
    }
}

#[async_trait]
impl Auth for AuthService {
    async fn generate_token(
        &self,
        email: &str,
        password: &str,
    ) -> Result<AuthenticationTokens, Error> {
        tracing::debug!(email, "Searching for user with email");

        let user = self
            .store
            .find_user_by_email(email)
            .await
            .map_err(Error::unknown)?
            .ok_or_else(|| Error::unauthorized(INVALID_CREDENTIALS))?;

        if utils::compare_hash_and_password(&user.password, password).is_err() {
            tracing::debug!("Password Comparison Failed");
            return Err(Error::unauthorized(INVALID_CREDENTIALS));
        }

        self.generate_tokens(&user)
    }

    async fn refresh_token(&self, refresh_token: &str) -> Result<AuthenticationTokens, Error> {
        let user_id = self.user_id_from(refresh_token)?;
        let user = self.require_user_by_id(user_id).await?;

        self.generate_tokens(&user)
    }

    async fn get_identity(&self, access_token: &str) -> Result<User, Error> {
        let user_id = self.user_id_from(access_token)?;

        self.require_user_by_id(user_id).await
    }

    async fn password_recovery(&self, email: &str, callback_uri: &str) -> Result<(), Error> {
        let user = self
            .store
            .find_user_by_email(email)
            .await
            .map_err(Error::unknown)?
            .ok_or_else(|| Error::not_found("invalid email address"))?;

        // Only one pending request per address is kept.
        self.store
            .delete_password_reset(email)
            .await
            .map_err(Error::unknown)?;

        let token = utils::random_token();

        // Only the digest is stored, the raw token travels by email.
        self.store
            .create_password_reset(email, &utils::hash_token(&token))
            .await
            .map_err(Error::unknown)?;

        let data = HashMap::from([
            ("username".to_owned(), user.user_name.clone()),
            ("callbackURI".to_owned(), format!("{callback_uri}{token}")),
        ]);

        self.mailer
            .send_html(&user.email, "Password Recovery", "password-reset", data)
            .await
            .map_err(Error::unknown)?;

        Ok(())
    }

    async fn reset_password(&self, token: &str, new_password: &str) -> Result<(), Error> {
        let password_reset = self
            .store
            .find_password_by_token(&utils::hash_token(token))
            .await
            .map_err(Error::unknown)?
            .ok_or_else(|| Error::bad_request("invalid or expired token"))?;

        if password_reset.created_at + Duration::minutes(PASSWORD_RESET_TTL_MINUTES) < Utc::now() {
            return Err(Error::bad_request("invalid or expired token"));
        }

        let hashed_password = utils::hash_password(new_password)?;

        self.store
            .reset_password(&password_reset.email, &hashed_password)
            .await
            .map_err(Error::unknown)?;

        self.store
            .delete_password_reset(&password_reset.email)
            .await
            .map_err(Error::unknown)?;

        Ok(())
    }
}
