//! Turns the crypto primitives in `package::crypto` into the error type the
//! rest of the application speaks.
//!
//! The mapping is the interesting part: a wrong password and an unreadable
//! stored hash both become the same 401, so a caller cannot tell the two apart
//! and use that to probe for accounts.

use crate::package::crypto;
use crate::shared::errdef::Error;

pub use crypto::random_token;

pub fn hash_password(password: &str) -> Result<String, Error> {
    crypto::hash_password(password).map_err(Error::unknown)
}

pub fn compare_hash_and_password(hashed: &str, password: &str) -> Result<(), Error> {
    crypto::verify_password(hashed, password).map_err(|error| match error {
        crypto::Error::Mismatch => Error::unauthorized("password mismatch"),
        other => Error::unauthorized("password mismatch").with_cause(other),
    })
}

pub fn hash_token(raw_token: &str) -> String {
    crypto::token_digest(raw_token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_a_hashed_password() {
        let hash = hash_password("Sup3r$ecret").expect("hashing should succeed");

        assert!(compare_hash_and_password(&hash, "Sup3r$ecret").is_ok());
        assert!(compare_hash_and_password(&hash, "wrong").is_err());
    }

    #[test]
    fn a_bad_password_and_a_bad_hash_look_the_same_to_the_caller() {
        let hash = hash_password("Sup3r$ecret").expect("hashing should succeed");

        let visible = |error: Error| match error {
            Error::App(app) => (app.code, app.message),
            Error::Validation(_) => panic!("a rejected password is not a validation error"),
        };

        let mismatch = visible(compare_hash_and_password(&hash, "wrong").expect_err("rejects"));
        let malformed =
            visible(compare_hash_and_password("not-a-hash", "Sup3r$ecret").expect_err("rejects"));

        assert_eq!(mismatch, malformed);
    }

    #[test]
    fn hashes_tokens_deterministically() {
        assert_eq!(hash_token("abc"), hash_token("abc"));
        assert_ne!(hash_token("abc"), hash_token("abd"));
        assert_ne!(random_token(), random_token());
    }
}
