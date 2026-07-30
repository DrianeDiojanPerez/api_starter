use rand::RngCore;
use sha2::{Digest, Sha256};

use crate::package::errdef::Error;

const BCRYPT_COST: u32 = bcrypt::DEFAULT_COST;

pub fn hash_password(password: &str) -> Result<String, Error> {
    bcrypt::hash(password, BCRYPT_COST).map_err(Error::unknown)
}

/// A wrong password and an unreadable stored hash return the same error, so a
/// caller cannot tell them apart and use the difference to probe for accounts.
pub fn compare_hash_and_password(hashed: &str, password: &str) -> Result<(), Error> {
    match bcrypt::verify(password, hashed) {
        Ok(true) => Ok(()),
        Ok(false) => Err(Error::unauthorized("password mismatch")),
        Err(error) => Err(Error::unauthorized("password mismatch").with_cause(error)),
    }
}

pub fn random_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// Only the digest is persisted, so a leaked table cannot be replayed.
pub fn hash_token(raw_token: &str) -> String {
    hex::encode(Sha256::digest(raw_token.as_bytes()))
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
    fn the_same_password_hashes_differently_every_time() {
        let first = hash_password("Sup3r$ecret").expect("hashing should succeed");
        let second = hash_password("Sup3r$ecret").expect("hashing should succeed");

        assert_ne!(first, second, "bcrypt salts each hash");
        assert!(compare_hash_and_password(&second, "Sup3r$ecret").is_ok());
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
    }

    #[test]
    fn every_token_is_different_and_long_enough_to_be_unguessable() {
        let token = random_token();

        assert_eq!(token.len(), 64, "32 bytes, hex encoded");
        assert_ne!(token, random_token());
    }
}
