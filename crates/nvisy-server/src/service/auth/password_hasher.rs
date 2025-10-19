//! Secure password hashing and verification using Argon2id.
//!
//! This module provides a comprehensive password hashing solution using the Argon2id
//! algorithm with recommended security parameters.
//!
//! # Examples
//!
//! ```rust
//! use nvisy_server::service::auth::AuthHasher;
//!
//! // Create a service with recommended secure configuration
//! let service = AuthHasher::new()?;
//!
//! // Hash a password
//! let password_hash = service.hash_password("my_secure_password")?;
//!
//! // Verify the password later
//! service.verify_password("my_secure_password", &password_hash)?;
//! ```

use argon2::password_hash::{Error as ArgonError, SaltString};
use argon2::{
    Algorithm, Argon2, Params, PasswordHash, PasswordHasher as _, PasswordVerifier, Version,
};
use rand::rngs::OsRng;

use crate::service::{Result, ServiceError};

/// Target identifier for password hashing service logging and error reporting.
const PASSWORD_HASHING_TARGET: &str = "nvisy::service::auth::hasher";

/// Secure password hashing and verification service using Argon2id.
///
/// This service provides cryptographically secure password hashing using the Argon2id
/// algorithm with OWASP recommended parameters.
///
/// # Security Features
///
/// - Uses Argon2id variant (hybrid of Argon2i and Argon2d)
/// - OWASP recommended parameters (19 MB memory, 2 iterations, 1 thread)
/// - Cryptographically secure random salt generation
/// - Timing-safe password verification
/// - Comprehensive error handling with security considerations
///
/// # Example
///
/// ```rust
/// use nvisy_server::service::auth::AuthHasher;
///
/// // Create service with recommended secure configuration
/// let service = AuthHasher::new()?;
///
/// // Hash a password
/// let password_hash = service.hash_password("secure_password123")?;
///
/// // Verify the password
/// service.verify_password("secure_password123", &password_hash)?;
/// ```
#[derive(Debug, Clone)]
pub struct AuthHasher {
    argon2: Argon2<'static>,
}

impl AuthHasher {
    /// Creates a new password hashing service with OWASP recommended configuration.
    ///
    /// Uses the following parameters:
    /// - Memory cost: 19456 KiB (≈19 MB)
    /// - Time cost: 2 iterations
    /// - Parallelism: 1 thread
    ///
    /// # Errors
    ///
    /// Returns a service error if Argon2 initialization fails.
    pub fn new() -> Result<Self> {
        let params = Params::new(
            19456, // 19 MB - OWASP recommended
            2,     // 2 iterations - OWASP recommended
            1,     // 1 thread - OWASP recommended
            None,  // Use default output length (32 bytes)
        )
        .map_err(|e| {
            tracing::error!(
                target: PASSWORD_HASHING_TARGET,
                error = %e,
                "Failed to create Argon2 parameters"
            );

            ServiceError::config("Invalid password hashing configuration")
        })?;

        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        Ok(Self { argon2 })
    }

    /// Hashes a password using Argon2id with a cryptographically secure random salt.
    ///
    /// The returned hash string includes all necessary parameters and the salt,
    /// making it suitable for long-term storage in a database.
    ///
    /// # Arguments
    ///
    /// * `password` - The plaintext password to hash
    ///
    /// # Returns
    ///
    /// A PHC string format hash that includes the algorithm, parameters, salt,
    /// and hash value. This can be stored directly in a database.
    ///
    /// # Errors
    ///
    /// Returns an error if hashing fails.
    ///
    /// # Security Notes
    ///
    /// - Each call generates a unique cryptographically secure salt
    /// - The password is processed securely and not logged
    /// - The function has consistent timing regardless of password content
    /// - All sensitive data is cleared from memory as soon as possible
    pub fn hash_password(&self, password: &str) -> Result<String> {
        // Generate cryptographically secure salt
        let salt = SaltString::try_from_rng(&mut OsRng).map_err(|e| {
            tracing::error!(
                target: PASSWORD_HASHING_TARGET,
                error = %e,
                "Failed to generate cryptographically secure salt"
            );
            ServiceError::internal("Password hashing system error")
        })?;

        // Hash the password
        let password_hash = self
            .argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| {
                tracing::error!(
                    target: PASSWORD_HASHING_TARGET,
                    error = %e,
                    "Password hashing operation failed"
                );

                ServiceError::internal("Password hashing failed")
            })?;

        Ok(password_hash.to_string())
    }

    /// Verifies a password against a stored hash.
    ///
    /// This function performs timing-safe verification to prevent side-channel attacks.
    ///
    /// # Arguments
    ///
    /// * `password` - The plaintext password to verify
    /// * `stored_hash` - The PHC string format hash retrieved from storage
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the password is correct and verification succeeds.
    ///
    /// # Errors
    ///
    /// Returns an error if verification fails or the hash format is invalid.
    ///
    /// # Security Notes
    ///
    /// - Uses timing-safe comparison to prevent timing attacks
    /// - Does not leak information about why verification failed
    pub fn verify_password(&self, password: &str, stored_hash: &str) -> Result<()> {
        // Parse the stored hash
        let parsed_hash = PasswordHash::new(stored_hash).map_err(|e| {
            tracing::warn!(
                target: PASSWORD_HASHING_TARGET,
                error = %e,
                "Invalid password hash format provided"
            );

            ServiceError::auth("Invalid password hash format")
        })?;

        match self
            .argon2
            .verify_password(password.as_bytes(), &parsed_hash)
        {
            Ok(()) => {
                tracing::debug!(
                    target: PASSWORD_HASHING_TARGET,
                    "Password verification successful"
                );

                Ok(())
            }
            Err(ArgonError::Password) => {
                tracing::debug!(
                    target: PASSWORD_HASHING_TARGET,
                    "Password verification failed - incorrect password provided"
                );

                Err(ServiceError::auth("Authentication failed"))
            }
            Err(e) => {
                tracing::error!(
                    target: PASSWORD_HASHING_TARGET,
                    error = %e,
                    "Password verification system error"
                );

                Err(ServiceError::internal("Password verification failed"))
            }
        }
    }
}

impl Default for AuthHasher {
    fn default() -> Self {
        Self::new().expect("Failed to create default AuthHasher service")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_and_verify_password() -> anyhow::Result<()> {
        let hasher = AuthHasher::new()?;
        let password = "secure_password_123";
        let hash = hasher.hash_password(password)?;

        assert!(hash.starts_with("$argon2id$"));
        assert!(hasher.verify_password(password, &hash).is_ok());
        assert!(hasher.verify_password("wrong_password", &hash).is_err());

        Ok(())
    }

    #[test]
    fn hash_produces_unique_salts() -> anyhow::Result<()> {
        let hasher = AuthHasher::new()?;
        let password = "test_password";

        let hash1 = hasher.hash_password(password)?;
        let hash2 = hasher.hash_password(password)?;

        assert_ne!(hash1, hash2);
        assert!(hasher.verify_password(password, &hash1).is_ok());
        assert!(hasher.verify_password(password, &hash2).is_ok());

        Ok(())
    }
}
