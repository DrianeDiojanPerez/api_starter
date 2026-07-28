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

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashMap;
    use std::sync::Mutex;

    use async_trait::async_trait;

    use crate::sdk::{MaskedBytes, PasswordReset};
    use crate::shared::emailer::EmailerError;
    use crate::shared::errdef::code;
    use crate::shared::jwt::HmacTokenGenerator;

    const PASSWORD: &str = "Sup3r$ecret";

    #[derive(Default)]
    struct FakeStore {
        users: Vec<User>,
        resets: Mutex<Vec<PasswordReset>>,
        password_updates: Mutex<Vec<(String, String)>>,
        deleted_resets: Mutex<Vec<String>>,
    }

    impl FakeStore {
        fn with_user(user: User) -> Self {
            Self {
                users: vec![user],
                ..Self::default()
            }
        }

        fn push_reset(&self, email: &str, token: &str, created_at: chrono::DateTime<Utc>) {
            self.resets.lock().unwrap().push(PasswordReset {
                email: email.to_owned(),
                token: token.to_owned(),
                created_at,
            });
        }
    }

    #[async_trait]
    impl Store for FakeStore {
        async fn find_user_by_id(&self, user_id: Uuid) -> Result<Option<User>, sqlx::Error> {
            Ok(self.users.iter().find(|u| u.id == user_id).cloned())
        }

        async fn find_user_by_email(&self, email: &str) -> Result<Option<User>, sqlx::Error> {
            Ok(self.users.iter().find(|u| u.email == email).cloned())
        }

        async fn create_password_reset(&self, email: &str, token: &str) -> Result<(), sqlx::Error> {
            self.push_reset(email, token, Utc::now());
            Ok(())
        }

        async fn reset_password(&self, email: &str, new_password: &str) -> Result<(), sqlx::Error> {
            self.password_updates
                .lock()
                .unwrap()
                .push((email.to_owned(), new_password.to_owned()));
            Ok(())
        }

        async fn find_password_by_token(
            &self,
            token: &str,
        ) -> Result<Option<PasswordReset>, sqlx::Error> {
            Ok(self
                .resets
                .lock()
                .unwrap()
                .iter()
                .find(|reset| reset.token == token)
                .cloned())
        }

        async fn delete_password_reset(&self, email: &str) -> Result<(), sqlx::Error> {
            self.deleted_resets.lock().unwrap().push(email.to_owned());
            self.resets.lock().unwrap().retain(|r| r.email != email);
            Ok(())
        }
    }

    /// to, subject, template name, template data.
    type SentMail = (String, String, String, HashMap<String, String>);

    #[derive(Default)]
    struct FakeMailer {
        sent: Mutex<Vec<SentMail>>,
    }

    #[async_trait]
    impl Emailer for FakeMailer {
        async fn send_html(
            &self,
            to: &str,
            subject: &str,
            template_name: &str,
            data: HashMap<String, String>,
        ) -> Result<(), EmailerError> {
            self.sent.lock().unwrap().push((
                to.to_owned(),
                subject.to_owned(),
                template_name.to_owned(),
                data,
            ));
            Ok(())
        }
    }

    fn a_user() -> User {
        User {
            id: Uuid::new_v4(),
            email: "admin@example.com".to_owned(),
            user_name: "admin".to_owned(),
            password: utils::hash_password(PASSWORD).expect("hashing should succeed"),
            roles: vec!["Admin".to_owned()],
        }
    }

    fn service_with(
        store: Arc<FakeStore>,
        mailer: Arc<FakeMailer>,
    ) -> (AuthService, Arc<dyn TokenGenerator>) {
        let jwt: Arc<dyn TokenGenerator> =
            Arc::new(HmacTokenGenerator::new(&MaskedBytes::new("secret")));

        (
            AuthService::new(jwt.clone(), store, mailer, 3600, 604_800),
            jwt,
        )
    }

    fn code_of(err: &Error) -> i32 {
        match err {
            Error::App(err) => err.code,
            Error::Validation(_) => code::VALIDATION_FAILED,
        }
    }

    #[tokio::test]
    async fn login_returns_a_token_pair() {
        let user = a_user();
        let store = Arc::new(FakeStore::with_user(user.clone()));
        let (service, jwt) = service_with(store, Arc::new(FakeMailer::default()));

        let tokens = service
            .generate_token(&user.email, PASSWORD)
            .await
            .expect("login should succeed");

        let claims = jwt
            .validate_token(&tokens.token)
            .expect("the access token should validate");

        assert_eq!(
            claims.get("user_id").and_then(Value::as_str),
            Some(user.id.to_string().as_str())
        );
        assert_eq!(
            claims.get("roles"),
            Some(&Value::from(vec!["Admin".to_owned()]))
        );

        // The refresh token carries the identity only, never the roles.
        let refresh_claims = jwt
            .validate_token(&tokens.refresh_token)
            .expect("the refresh token should validate");
        assert!(!refresh_claims.contains_key("roles"));
    }

    #[tokio::test]
    async fn login_rejects_a_wrong_password() {
        let user = a_user();
        let store = Arc::new(FakeStore::with_user(user.clone()));
        let (service, _) = service_with(store, Arc::new(FakeMailer::default()));

        let err = service
            .generate_token(&user.email, "not the password")
            .await
            .expect_err("login should fail");

        assert_eq!(code_of(&err), code::UNAUTHORIZED);
        assert!(err.to_string().contains(INVALID_CREDENTIALS));
    }

    #[tokio::test]
    async fn login_does_not_reveal_whether_the_email_exists() {
        let store = Arc::new(FakeStore::with_user(a_user()));
        let (service, _) = service_with(store, Arc::new(FakeMailer::default()));

        let unknown = service
            .generate_token("nobody@example.com", PASSWORD)
            .await
            .expect_err("login should fail");
        let wrong_password = service
            .generate_token("admin@example.com", "wrong")
            .await
            .expect_err("login should fail");

        assert_eq!(unknown.to_string(), wrong_password.to_string());
    }

    #[tokio::test]
    async fn refresh_issues_a_new_pair() {
        let user = a_user();
        let store = Arc::new(FakeStore::with_user(user.clone()));
        let (service, _) = service_with(store, Arc::new(FakeMailer::default()));

        let tokens = service
            .generate_token(&user.email, PASSWORD)
            .await
            .expect("login should succeed");

        let refreshed = service
            .refresh_token(&tokens.refresh_token)
            .await
            .expect("refresh should succeed");

        assert!(!refreshed.token.is_empty());
        assert!(!refreshed.refresh_token.is_empty());
    }

    #[tokio::test]
    async fn refresh_rejects_a_malformed_token() {
        let store = Arc::new(FakeStore::with_user(a_user()));
        let (service, _) = service_with(store, Arc::new(FakeMailer::default()));

        let err = service
            .refresh_token("not.a.token")
            .await
            .expect_err("refresh should fail");

        assert_eq!(code_of(&err), code::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn refresh_rejects_a_token_for_a_deleted_user() {
        let user = a_user();
        let store = Arc::new(FakeStore::with_user(user.clone()));
        let (service, jwt) = service_with(store, Arc::new(FakeMailer::default()));

        let orphan = jwt
            .generate_token(
                Claims::from([(
                    "user_id".to_owned(),
                    Value::from(Uuid::new_v4().to_string()),
                )]),
                Utc::now().timestamp() + 3600,
            )
            .expect("token should be signed");

        let err = service
            .refresh_token(&orphan)
            .await
            .expect_err("refresh should fail");

        assert_eq!(code_of(&err), code::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn get_identity_resolves_the_user_behind_a_token() {
        let user = a_user();
        let store = Arc::new(FakeStore::with_user(user.clone()));
        let (service, _) = service_with(store, Arc::new(FakeMailer::default()));

        let tokens = service
            .generate_token(&user.email, PASSWORD)
            .await
            .expect("login should succeed");

        let identity = service
            .get_identity(&tokens.token)
            .await
            .expect("identity should resolve");

        assert_eq!(identity.id, user.id);
        assert_eq!(identity.roles, vec!["Admin".to_owned()]);
    }

    #[tokio::test]
    async fn password_recovery_stores_a_digest_and_mails_the_raw_token() {
        let user = a_user();
        let store = Arc::new(FakeStore::with_user(user.clone()));
        let mailer = Arc::new(FakeMailer::default());
        let (service, _) = service_with(store.clone(), mailer.clone());

        service
            .password_recovery(&user.email, "https://example.com/reset?token=")
            .await
            .expect("recovery should succeed");

        let sent = mailer.sent.lock().unwrap();
        let (to, subject, template, data) = sent.first().expect("a mail should have been sent");

        assert_eq!(to, &user.email);
        assert_eq!(subject, "Password Recovery");
        assert_eq!(template, "password-reset");
        assert_eq!(data.get("username"), Some(&user.user_name));

        let raw_token = data
            .get("callbackURI")
            .and_then(|uri| uri.split("token=").nth(1))
            .expect("the callback should carry the token");

        let stored = store.resets.lock().unwrap();
        let reset = stored.first().expect("a reset should be stored");

        assert_ne!(reset.token, raw_token, "the raw token must not be stored");
        assert_eq!(reset.token, utils::hash_token(raw_token));
    }

    #[tokio::test]
    async fn password_recovery_clears_a_previous_request() {
        let user = a_user();
        let store = Arc::new(FakeStore::with_user(user.clone()));
        let (service, _) = service_with(store.clone(), Arc::new(FakeMailer::default()));

        store.push_reset(&user.email, "stale", Utc::now());

        service
            .password_recovery(&user.email, "https://example.com/reset?token=")
            .await
            .expect("recovery should succeed");

        assert_eq!(store.deleted_resets.lock().unwrap().len(), 1);
        assert_eq!(store.resets.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn password_recovery_reports_an_unknown_address() {
        let store = Arc::new(FakeStore::with_user(a_user()));
        let mailer = Arc::new(FakeMailer::default());
        let (service, _) = service_with(store, mailer.clone());

        let err = service
            .password_recovery("nobody@example.com", "https://example.com/")
            .await
            .expect_err("recovery should fail");

        assert_eq!(code_of(&err), code::NOT_FOUND);
        assert!(mailer.sent.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn reset_password_updates_the_hash_and_consumes_the_token() {
        let user = a_user();
        let store = Arc::new(FakeStore::with_user(user.clone()));
        let (service, _) = service_with(store.clone(), Arc::new(FakeMailer::default()));

        store.push_reset(&user.email, &utils::hash_token("raw-token"), Utc::now());

        service
            .reset_password("raw-token", "N3wP@ssword")
            .await
            .expect("reset should succeed");

        let updates = store.password_updates.lock().unwrap();
        let (email, hash) = updates.first().expect("the password should be updated");

        assert_eq!(email, &user.email);
        assert!(utils::compare_hash_and_password(hash, "N3wP@ssword").is_ok());
        assert!(store.resets.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn reset_password_rejects_an_expired_token() {
        let user = a_user();
        let store = Arc::new(FakeStore::with_user(user.clone()));
        let (service, _) = service_with(store.clone(), Arc::new(FakeMailer::default()));

        store.push_reset(
            &user.email,
            &utils::hash_token("raw-token"),
            Utc::now() - Duration::minutes(PASSWORD_RESET_TTL_MINUTES + 1),
        );

        let err = service
            .reset_password("raw-token", "N3wP@ssword")
            .await
            .expect_err("reset should fail");

        assert_eq!(code_of(&err), code::BAD_REQUEST);
        assert!(store.password_updates.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn reset_password_rejects_an_unknown_token() {
        let store = Arc::new(FakeStore::with_user(a_user()));
        let (service, _) = service_with(store.clone(), Arc::new(FakeMailer::default()));

        let err = service
            .reset_password("never-issued", "N3wP@ssword")
            .await
            .expect_err("reset should fail");

        assert_eq!(code_of(&err), code::BAD_REQUEST);
        assert!(store.password_updates.lock().unwrap().is_empty());
    }
}
