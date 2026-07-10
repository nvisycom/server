//! Password handling: strength validation and Argon2id hashing in one service.
//!
//! Setting a password is always "validate, then hash", and verifying is
//! "hash-compare or spend equivalent time"; this service owns both halves so
//! callers reach for a single dependency instead of pairing a validator with a
//! hasher and remembering to run them in order.

use super::password_hasher::PasswordHasher;
use super::password_strength::PasswordStrength;
use crate::handler::Result;

/// Password strength validation and Argon2id hashing.
#[derive(Debug, Clone, Default)]
pub struct PasswordService {
    hasher: PasswordHasher,
    strength: PasswordStrength,
}

impl PasswordService {
    /// Creates a new password service.
    pub fn new() -> Self {
        Self {
            hasher: PasswordHasher::new(),
            strength: PasswordStrength::new(),
        }
    }

    /// Validates a password's strength against the configured policy.
    ///
    /// `user_inputs` are values (email, name) the password must not resemble.
    pub fn validate(&self, password: &str, user_inputs: &[&str]) -> Result<()> {
        self.strength.validate_password(password, user_inputs)
    }

    /// Validates strength, then hashes the password with Argon2id.
    ///
    /// The single entry point for accepting a new password: it can't be hashed
    /// without first passing the strength check.
    pub fn validate_and_hash(&self, password: &str, user_inputs: &[&str]) -> Result<String> {
        self.strength.validate_password(password, user_inputs)?;
        self.hasher.hash_password(password)
    }

    /// Hashes a password with Argon2id, without a strength check.
    pub fn hash(&self, password: &str) -> Result<String> {
        self.hasher.hash_password(password)
    }

    /// Verifies a password against a stored Argon2id hash.
    pub fn verify(&self, password: &str, stored_hash: &str) -> Result<()> {
        self.hasher.verify_password(password, stored_hash)
    }

    /// Runs a dummy verification to keep login timing constant when the account
    /// does not exist, defeating account enumeration by timing.
    pub fn verify_dummy(&self, password: &str) -> bool {
        self.hasher.verify_dummy_password(password)
    }
}
