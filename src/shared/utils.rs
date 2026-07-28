use rand::RngCore;
use sha2::{Digest, Sha256};

use crate::shared::errdef::Error;

/// Cost matches the Go service so hashes stay interchangeable between both
/// implementations during a migration.
const BCRYPT_COST: u32 = bcrypt::DEFAULT_COST;

pub fn hash_password(password: &str) -> Result<String, Error> {
    bcrypt::hash(password, BCRYPT_COST).map_err(Error::unknown)
}

pub fn compare_hash_and_password(hashed: &str, password: &str) -> Result<(), Error> {
    match bcrypt::verify(password, hashed) {
        Ok(true) => Ok(()),
        Ok(false) => Err(Error::unauthorized("password mismatch")),
        Err(err) => Err(Error::unauthorized("password mismatch").with_cause(err)),
    }
}

/// 32 random bytes, hex encoded. This is the value mailed to the user.
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
    fn hashes_tokens_deterministically() {
        assert_eq!(hash_token("abc"), hash_token("abc"));
        assert_ne!(hash_token("abc"), hash_token("abd"));
        assert_ne!(random_token(), random_token());
    }
}
