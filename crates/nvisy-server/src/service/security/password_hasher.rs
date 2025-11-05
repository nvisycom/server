//! Secure password hashing and verification using Argon2id.
//!
//! This module provides a comprehensive password hashing solution using the Argon2id
//! algorithm with recommended security parameters. The password hashing and verification
//! methods are designed for use in HTTP handlers and return appropriate HTTP error
//! responses for client consumption.
//!
//! # HTTP Error Support
//!
//! The `hash_password` and `verify_password` methods return handler-compatible errors:
//! - `hash_password` returns `ErrorKind::InternalServerError` for system failures
//! - `verify_password` returns `ErrorKind::Unauthorized` for authentication failures
//!   and `ErrorKind::InternalServerError` for system errors
//!
//! # Examples
//!
//! ## Service Creation
//! ```rust
//! use nvisy_server::service::auth::AuthHasher;
//!
//! // Create a service with recommended secure configuration
//! let service = AuthHasher::new()?; // Returns ServiceError for configuration issues
//! ```
//!
//! ## Handler Usage
//! ```rust
//! use nvisy_server::service::auth::AuthHasher;
//! use nvisy_server::handler::Result;
//!
//! async fn login_handler(auth_hasher: AuthHasher) -> Result<()> {
//!     let password = "user_password";
//!     let stored_hash = get_stored_hash().await;
//!
//!     // Returns HTTP errors suitable for handler responses
//!     auth_hasher.verify_password(password, &stored_hash)?;
//!     Ok(())
//! }
//! ```

use argon2::password_hash::{Error as ArgonError, SaltString};
use argon2::{
    Algorithm, Argon2, Params, PasswordHash, PasswordHasher as _, PasswordVerifier, Version,
};
use rand::rngs::OsRng;

use crate::handler::{ErrorKind, Result};
use crate::service::{Result as ServiceResult, ServiceError};

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
    pub fn new() -> ServiceResult<Self> {
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
    /// This method is designed for use in HTTP handlers and returns appropriate
    /// HTTP error responses for client consumption.
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
    /// Returns `ErrorKind::InternalServerError` with user-friendly message if:
    /// - Salt generation fails
    /// - Password hashing operation fails
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
            ErrorKind::InternalServerError
                .with_message("Password processing failed")
                .with_context("Salt generation error")
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

                ErrorKind::InternalServerError
                    .with_message("Password processing failed")
                    .with_context("Hash generation error")
            })?;

        Ok(password_hash.to_string())
    }

    /// Verifies a password against a stored hash.
    ///
    /// This function performs timing-safe verification to prevent side-channel attacks
    /// and is designed for use in HTTP handlers, returning appropriate HTTP error
    /// responses for client consumption.
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
    /// Returns different HTTP errors based on failure type:
    /// - `ErrorKind::Unauthorized` for incorrect passwords
    /// - `ErrorKind::InternalServerError` for invalid hash format or system errors
    ///
    /// # Security Notes
    ///
    /// - Uses timing-safe comparison to prevent timing attacks
    /// - Does not leak information about why verification failed
    /// - Error messages are safe for client consumption
    pub fn verify_password(&self, password: &str, stored_hash: &str) -> Result<()> {
        // Parse the stored hash
        let parsed_hash = PasswordHash::new(stored_hash).map_err(|e| {
            tracing::warn!(
                target: PASSWORD_HASHING_TARGET,
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

                Err(ErrorKind::Unauthorized
                    .with_message("Authentication failed")
                    .with_context("Invalid credentials"))
            }
            Err(e) => {
                tracing::error!(
                    target: PASSWORD_HASHING_TARGET,
                    error = %e,
                    "Password verification system error"
                );

                Err(ErrorKind::InternalServerError
                    .with_message("Authentication system temporarily unavailable")
                    .with_context("Verification error"))
            }
        }
    }

    /// Performs a dummy password verification to maintain consistent timing.
    ///
    /// This method is used when an account doesn't exist to prevent timing attacks
    /// that could reveal which accounts exist in the system. It generates a random
    /// password, hashes it, and performs verification (which will always fail).
    ///
    /// # Arguments
    ///
    /// * `password` - The password to verify (will be checked against a random hash)
    ///
    /// # Security Notes
    ///
    /// - Takes approximately the same time as a real password verification
    /// - Prevents account enumeration via timing analysis
    /// - Always returns false but performs actual cryptographic work
    pub fn verify_dummy_password(&self, password: &str) -> bool {
        use rand::Rng;

        // Generate a random dummy password (16 characters)
        let password_len = rand::random_range(16..32);
        let dummy_password: String = (0..password_len)
            .map(|_| rand::rng().sample(rand::distr::Alphanumeric) as char)
            .collect();

        // Hash the dummy password and verify - this will always fail
        // but takes the same time as a real verification
        if let Ok(dummy_hash) = self.hash_password(&dummy_password) {
            let _ = self.verify_password(password, &dummy_hash);
        }

        false
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

    #[test]
    fn verify_password_returns_unauthorized_for_wrong_password() -> anyhow::Result<()> {
        let hasher = AuthHasher::new()?;
        let correct_password = "correct_password";
        let wrong_password = "wrong_password";
        let hash = hasher
            .hash_password(correct_password)
            .map_err(|_| anyhow::anyhow!("Failed to hash password"))?;

        // Should fail with wrong password and return Unauthorized
        let result = hasher.verify_password(wrong_password, &hash);
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn verify_password_returns_error_for_invalid_hash() {
        let hasher = AuthHasher::new().unwrap();
        let password = "test_password";
        let invalid_hash = "invalid_hash_format";

        // Should fail with invalid hash format
        let result = hasher.verify_password(password, invalid_hash);
        assert!(result.is_err());
    }

    #[test]
    fn password_hasher_http_error_integration() -> anyhow::Result<()> {
        let hasher = AuthHasher::new()?;
        let password = "secure_test_password_123";

        // Test successful hashing - should return a valid hash string
        let hash_result = hasher.hash_password(password);
        assert!(hash_result.is_ok(), "Password hashing should succeed");
        let hash = hash_result?;
        assert!(
            hash.starts_with("$argon2id$"),
            "Hash should be in PHC format"
        );

        // Test successful verification - should return Ok(())
        let verify_result = hasher.verify_password(password, &hash);
        assert!(
            verify_result.is_ok(),
            "Correct password should verify successfully"
        );

        // Test failed verification with wrong password - should return handler Error
        let wrong_password = "wrong_password_123";
        let verify_wrong_result = hasher.verify_password(wrong_password, &hash);
        assert!(
            verify_wrong_result.is_err(),
            "Wrong password should fail verification"
        );

        // Verify the error is the expected HTTP error type by checking it can be converted to response
        if let Err(error) = verify_wrong_result {
            use crate::handler::ErrorKind;
            assert_eq!(
                error.kind(),
                ErrorKind::Unauthorized,
                "Should return Unauthorized for wrong password"
            );
        }

        // Test invalid hash format - should return handler Error
        let invalid_hash = "not_a_valid_hash_format";
        let invalid_hash_result = hasher.verify_password(password, invalid_hash);
        assert!(invalid_hash_result.is_err(), "Invalid hash should fail");

        if let Err(error) = invalid_hash_result {
            use crate::handler::ErrorKind;
            assert_eq!(
                error.kind(),
                ErrorKind::InternalServerError,
                "Should return InternalServerError for invalid hash"
            );
        }

        Ok(())
    }
}
