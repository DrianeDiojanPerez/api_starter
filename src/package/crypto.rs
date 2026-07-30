//! Password hashing and token generation.
//!
//! These know nothing about HTTP status codes or the application's error
//! envelope. Deciding that a password mismatch is a 401 is a policy call, so
//! it is made by the caller in `shared::utils`, not here.

use rand::RngCore;
use sha2::{Digest, Sha256};

/// Cost matches the Go service so hashes stay interchangeable between both
/// implementations during a migration.
const BCRYPT_COST: u32 = bcrypt::DEFAULT_COST;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not hash the password: {0}")]
    Hash(#[source] bcrypt::BcryptError),
    #[error("the stored hash could not be read: {0}")]
    Malformed(#[source] bcrypt::BcryptError),
    #[error("the password does not match the stored hash")]
    Mismatch,
}

pub fn hash_password(password: &str) -> Result<String, Error> {
    bcrypt::hash(password, BCRYPT_COST).map_err(Error::Hash)
}

pub fn verify_password(hashed: &str, password: &str) -> Result<(), Error> {
    match bcrypt::verify(password, hashed) {
        Ok(true) => Ok(()),
        Ok(false) => Err(Error::Mismatch),
        Err(error) => Err(Error::Malformed(error)),
    }
}

pub fn random_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// Only the digest is persisted, so a leaked table cannot be replayed.
pub fn token_digest(raw_token: &str) -> String {
    hex::encode(Sha256::digest(raw_token.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_a_hashed_password() {
        let hash = hash_password("Sup3r$ecret").expect("hashing should succeed");

        assert!(verify_password(&hash, "Sup3r$ecret").is_ok());
        assert!(matches!(
            verify_password(&hash, "wrong"),
            Err(Error::Mismatch)
        ));
    }

    #[test]
    fn the_same_password_hashes_differently_every_time() {
        let first = hash_password("Sup3r$ecret").expect("hashing should succeed");
        let second = hash_password("Sup3r$ecret").expect("hashing should succeed");

        assert_ne!(first, second, "bcrypt salts each hash");
        assert!(verify_password(&second, "Sup3r$ecret").is_ok());
    }

    #[test]
    fn a_hash_that_is_not_a_hash_is_reported_as_malformed() {
        assert!(matches!(
            verify_password("not-a-bcrypt-hash", "Sup3r$ecret"),
            Err(Error::Malformed(_))
        ));
    }

    #[test]
    fn digests_tokens_deterministically() {
        assert_eq!(token_digest("abc"), token_digest("abc"));
        assert_ne!(token_digest("abc"), token_digest("abd"));
    }

    #[test]
    fn every_token_is_different_and_long_enough_to_be_unguessable() {
        let token = random_token();

        assert_eq!(token.len(), 64, "32 bytes, hex encoded");
        assert_ne!(token, random_token());
    }
}
