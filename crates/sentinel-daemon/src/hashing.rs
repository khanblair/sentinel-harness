//! Device tokens: generation, hashing, and comparison.
//!
//! The daemon keeps only a hash of each device token, so a store leak does not give
//! anyone access. A new token is the raw bytes of two random v4 UUIDs, hex encoded.

use sha2::{Digest, Sha256};
use uuid::Uuid;

/// A fresh device token: 64 hex characters from 32 random bytes.
pub fn new_token() -> String {
    let uuid1 = Uuid::new_v4();
    let uuid2 = Uuid::new_v4();
    let mut bytes = Vec::with_capacity(32);
    bytes.extend_from_slice(uuid1.as_bytes());
    bytes.extend_from_slice(uuid2.as_bytes());
    hex::encode(bytes)
}

/// SHA-256 of a token, lowercase hex. Only this value reaches the store.
pub fn hash_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

/// An equality check that takes the same time for every input of a given length.
pub fn constant_time_eq(a: &str, b: &str) -> bool {
    let a = a.as_bytes();
    let b = b.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    let mut difference: u8 = 0;
    for (left, right) in a.iter().zip(b.iter()) {
        difference |= left ^ right;
    }
    difference == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_token_is_64_hex_characters() {
        let token = new_token();
        assert_eq!(token.len(), 64);
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn two_new_tokens_differ() {
        assert_ne!(new_token(), new_token());
    }

    #[test]
    fn hash_token_is_stable() {
        assert_eq!(hash_token("abc"), hash_token("abc"));
    }

    #[test]
    fn hash_token_is_64_lowercase_hex_characters() {
        let hashed = hash_token("abc");
        assert_eq!(hashed.len(), 64);
        assert_eq!(hashed, hashed.to_ascii_lowercase());
        assert!(hashed.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn hash_token_matches_the_known_sha_256_of_abc() {
        assert_eq!(
            hash_token("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn constant_time_eq_matches_only_identical_tokens() {
        assert!(constant_time_eq("same", "same"));
        assert!(!constant_time_eq("same", "sane"));
        assert!(!constant_time_eq("same", "sam"));
        assert!(!constant_time_eq("short", "longer"));
    }
}
