//! MinIO authentication credentials.
//!
//! This module provides structures for handling MinIO authentication,
//! including access keys, secret keys, and session tokens.

use minio::s3::creds::StaticProvider;
use serde::{Deserialize, Serialize};

/// MinIO authentication credentials.
///
/// This struct encapsulates the authentication information required to connect
/// to a MinIO server, including access key, secret key, and optional session token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinioCredentials {
    /// Access key for MinIO authentication.
    pub access_key: String,

    /// Secret key for MinIO authentication.
    /// This field is marked as sensitive and will be masked in debug output.
    #[serde(skip_serializing)]
    pub secret_key: String,

    /// Optional session token for temporary credentials.
    /// Used with AWS STS or similar temporary credential systems.
    pub session_token: Option<String>,
}

impl MinioCredentials {
    /// Creates new MinIO credentials with access key and secret key.
    ///
    /// # Arguments
    ///
    /// * `access_key` - The MinIO access key
    /// * `secret_key` - The MinIO secret key
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nvisy_minio::MinioCredentials;
    ///
    /// let credentials = MinioCredentials::new(
    ///     "AKIAIOSFODNN7EXAMPLE",
    ///     "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
    /// );
    /// ```
    pub fn new(access_key: impl Into<String>, secret_key: impl Into<String>) -> Self {
        Self {
            access_key: access_key.into(),
            secret_key: secret_key.into(),
            session_token: None,
        }
    }

    /// Creates new MinIO credentials with access key, secret key, and session token.
    ///
    /// # Arguments
    ///
    /// * `access_key` - The MinIO access key
    /// * `secret_key` - The MinIO secret key
    /// * `session_token` - The session token for temporary credentials
    pub fn with_session_token(
        access_key: impl Into<String>,
        secret_key: impl Into<String>,
        session_token: impl Into<String>,
    ) -> Self {
        Self {
            access_key: access_key.into(),
            secret_key: secret_key.into(),
            session_token: Some(session_token.into()),
        }
    }

    /// Returns the access key.
    #[inline]
    pub fn access_key(&self) -> &str {
        &self.access_key
    }

    /// Returns the secret key.
    #[inline]
    pub fn secret_key(&self) -> &str {
        &self.secret_key
    }

    /// Returns the session token if available.
    #[inline]
    pub fn session_token(&self) -> Option<&str> {
        self.session_token.as_deref()
    }

    /// Returns a masked version of the access key for logging.
    ///
    /// This shows only the first 4 characters followed by asterisks.
    pub fn access_key_masked(&self) -> String {
        if self.access_key.len() <= 4 {
            "*".repeat(self.access_key.len())
        } else {
            format!("{}***", &self.access_key[..4])
        }
    }
}

impl From<MinioCredentials> for StaticProvider {
    fn from(credentials: MinioCredentials) -> Self {
        StaticProvider::new(
            &credentials.access_key,
            &credentials.secret_key,
            credentials.session_token.as_deref(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credentials_new() {
        let creds = MinioCredentials::new("access", "secret");
        assert_eq!(creds.access_key(), "access");
        assert_eq!(creds.secret_key(), "secret");
        assert!(creds.session_token().is_none());
    }

    #[test]
    fn test_credentials_with_session_token() {
        let creds = MinioCredentials::with_session_token("access", "secret", "token");
        assert_eq!(creds.access_key(), "access");
        assert_eq!(creds.secret_key(), "secret");
        assert_eq!(creds.session_token(), Some("token"));
    }

    #[test]
    fn test_credentials_masking() {
        let creds = MinioCredentials::new("AKIATEST12345", "secret");
        assert_eq!(creds.access_key_masked(), "AKIA***");

        let short_creds = MinioCredentials::new("ABC", "secret");
        assert_eq!(short_creds.access_key_masked(), "***");
    }
}
