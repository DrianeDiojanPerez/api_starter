use std::collections::BTreeMap;

use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde_json::Value;

use crate::sdk::MaskedBytes;

pub type Claims = BTreeMap<String, Value>;

#[derive(Debug, thiserror::Error)]
pub enum JwtError {
    #[error("error encountered when signing jwt token: {0}")]
    Sign(jsonwebtoken::errors::Error),
    #[error("error parsing jwt token: {0}")]
    Parse(jsonwebtoken::errors::Error),
}

pub trait TokenGenerator: Send + Sync {
    /// `expires_at` is an absolute unix timestamp, matching the Go signature.
    fn generate_token(&self, claims: Claims, expires_at: i64) -> Result<String, JwtError>;
    fn validate_token(&self, token: &str) -> Result<Claims, JwtError>;
}

pub struct HmacTokenGenerator {
    encoding: EncodingKey,
    decoding: DecodingKey,
}

impl HmacTokenGenerator {
    pub fn new(secret: &MaskedBytes) -> Self {
        Self {
            encoding: EncodingKey::from_secret(secret.expose()),
            decoding: DecodingKey::from_secret(secret.expose()),
        }
    }
}

impl TokenGenerator for HmacTokenGenerator {
    fn generate_token(&self, claims: Claims, expires_at: i64) -> Result<String, JwtError> {
        let mut claims = claims;
        claims.insert("exp".to_owned(), Value::from(expires_at));

        jsonwebtoken::encode(&Header::new(Algorithm::HS256), &claims, &self.encoding)
            .map_err(JwtError::Sign)
    }

    fn validate_token(&self, token: &str) -> Result<Claims, JwtError> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_required_spec_claims(&["exp"]);
        // No grace period on expiry, matching the Go implementation.
        validation.leeway = 0;

        let data = jsonwebtoken::decode::<Claims>(token, &self.decoding, &validation)
            .map_err(JwtError::Parse)?;

        let mut claims = data.claims;
        claims.remove("exp");

        Ok(claims)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generator() -> HmacTokenGenerator {
        HmacTokenGenerator::new(&MaskedBytes::new("secret"))
    }

    fn in_an_hour() -> i64 {
        chrono::Utc::now().timestamp() + 3600
    }

    #[test]
    fn round_trips_claims() {
        let generator = generator();
        let claims = Claims::from([("user_id".to_owned(), Value::from("abc"))]);

        let token = generator
            .generate_token(claims, in_an_hour())
            .expect("token should be signed");
        let decoded = generator
            .validate_token(&token)
            .expect("token should validate");

        assert_eq!(decoded.get("user_id"), Some(&Value::from("abc")));
        assert!(!decoded.contains_key("exp"));
    }

    #[test]
    fn rejects_an_expired_token() {
        let generator = generator();
        let token = generator
            .generate_token(Claims::new(), chrono::Utc::now().timestamp() - 60)
            .expect("token should be signed");

        assert!(generator.validate_token(&token).is_err());
    }

    #[test]
    fn rejects_a_token_signed_with_another_secret() {
        let token = HmacTokenGenerator::new(&MaskedBytes::new("other"))
            .generate_token(Claims::new(), in_an_hour())
            .expect("token should be signed");

        assert!(generator().validate_token(&token).is_err());
    }
}
