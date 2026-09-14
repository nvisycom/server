//! Argon2id password hashing and verification.

use argon2::password_hash::{Error as ArgonError, PasswordHasher as _, PasswordVerifier};
use argon2::{Argon2, PasswordHash};
use rand::distr::Alphanumeric;

use crate::response::{ErrorKind, Result};

/// Tracing target for password hashing operations.
const TRACING_TARGET: &str = "nvisy_server::service::password";

/// Hashes and verifies passwords with Argon2id.
#[derive(Debug, Clone)]
pub struct PasswordHasher {
    argon2: Argon2<'static>,
}

impl PasswordHasher {
    /// A hasher with the default Argon2id parameters.
    #[must_use]
    pub fn new() -> Self {
        Self {
            argon2: Argon2::default(),
        }
    }

    /// Hashes `password` with a fresh random salt, returning a PHC string (algorithm,
    /// parameters, salt, and hash) suitable for storage.
    ///
    /// # Errors
    ///
    /// `InternalServerError` if hashing fails.
    pub fn hash_password(&self, password: &str) -> Result<String> {
        let password_hash = self
            .argon2
            .hash_password(password.as_bytes())
            .map_err(|e| {
                tracing::error!(
                    target: TRACING_TARGET,
                    error = %e,
                    "password hashing operation failed"
                );

                ErrorKind::InternalServerError
                    .with_message("Password processing failed")
                    .with_context("Hash generation error")
            })?;

        Ok(password_hash.to_string())
    }

    /// Verifies `password` against a stored PHC hash, in constant time.
    ///
    /// # Errors
    ///
    /// - `Unauthorized` if the password does not match.
    /// - `InternalServerError` if the stored hash is malformed or verification
    ///   otherwise fails.
    pub fn verify_password(&self, password: &str, stored_hash: &str) -> Result<()> {
        // Parse the stored hash
        let parsed_hash = PasswordHash::new(stored_hash).map_err(|e| {
            tracing::warn!(
                target: TRACING_TARGET,
                error = %e,
                "Invalid password hash format provided"
            );

            ErrorKind::InternalServerError
                .with_message("Authentication system temporarily unavailable")
                .with_context("Hash format error")
        })?;

        match self
            .argon2
            .verify_password(password.as_bytes(), &parsed_hash)
        {
            Ok(()) => {
                tracing::debug!(
                    target: TRACING_TARGET,
                    "Password verification successful"
                );

                Ok(())
            }
            Err(ArgonError::PasswordInvalid) => {
                // TODO: Log account id.
                tracing::debug!(
                    target: TRACING_TARGET,
                    "Password verification failed: incorrect password provided"
                );

                Err(ErrorKind::Unauthorized
                    .with_message("Authentication failed")
                    .with_context("Invalid credentials"))
            }
            Err(e) => {
                tracing::error!(
                    target: TRACING_TARGET,
                    error = %e,
                    "Password verification system error"
                );

                Err(ErrorKind::InternalServerError
                    .with_message("Authentication temporarily unavailable")
                    .with_context("Verification error"))
            }
        }
    }

    /// Does the cryptographic work of a verification against a throwaway hash and
    /// returns `false`, so a login for a non-existent account takes the same time
    /// as a real one — defeating account enumeration by timing.
    #[must_use]
    pub fn verify_dummy_password(&self, password: &str) -> bool {
        use rand::RngExt;

        let mut rng = rand::rng();
        let dummy: String = (0..rand::random_range(16..32))
            .map(|_| rng.sample(Alphanumeric) as char)
            .collect();

        // Hash a random password and verify against it; the compare always fails
        // but spends the same time as a real verification.
        if let Ok(dummy_hash) = self.hash_password(&dummy) {
            let _ = self.verify_password(password, &dummy_hash);
        }
        false
    }
}

impl Default for PasswordHasher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Our error mapping: a wrong password verifies to `Unauthorized`, while a
    /// malformed stored hash is an internal fault (`InternalServerError`).
    #[test]
    fn verify_maps_failures_to_our_error_kinds() -> anyhow::Result<()> {
        let hasher = PasswordHasher::new();
        let hash = hasher.hash_password("correct-horse")?;

        let wrong = hasher.verify_password("wrong", &hash).unwrap_err();
        assert_eq!(wrong.kind(), ErrorKind::Unauthorized);

        let malformed = hasher
            .verify_password("correct-horse", "not-a-hash")
            .unwrap_err();
        assert_eq!(malformed.kind(), ErrorKind::InternalServerError);

        Ok(())
    }
}
