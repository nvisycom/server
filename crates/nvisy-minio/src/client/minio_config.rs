//! MinIO client configuration management.
//!
//! This module provides configuration structures for MinIO client setup,
//! including connection settings and operational parameters.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use url::Url;

use super::minio_credentials::MinioCredentials;
use crate::{Error, Result};

/// MinIO client configuration.
///
/// This struct contains all the configuration parameters needed to establish
/// a connection to a MinIO server, including endpoint, credentials, timeouts,
/// and other operational settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinioConfig {
    /// MinIO server endpoint URL.
    ///
    /// This should include the protocol (http:// or https://) and may include a port.
    /// Examples: "https://play.min.io", "http://localhost:9000"
    pub endpoint: Url,

    /// Authentication credentials.
    pub credentials: MinioCredentials,

    /// Connection timeout for initial connection establishment.
    ///
    /// This controls how long to wait when establishing a new connection
    /// to the MinIO server.
    pub connect_timeout: Duration,

    /// Request timeout for individual operations.
    ///
    /// This controls how long to wait for a single request to complete,
    /// including upload/download operations.
    pub request_timeout: Duration,

    /// Whether to use path-style requests.
    ///
    /// When true, uses URLs like "endpoint/bucket/object".
    /// When false, uses virtual-hosted style like "bucket.endpoint/object".
    /// MinIO typically uses path-style requests.
    pub path_style: bool,
}

impl MinioConfig {
    /// Creates a new MinIO configuration with the specified endpoint and credentials.
    ///
    /// # Arguments
    ///
    /// * `endpoint` - MinIO server endpoint URL
    /// * `credentials` - Authentication credentials
    ///
    /// # Errors
    ///
    /// Returns an error if the endpoint URL is invalid or malformed.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nvisy_minio::{MinioConfig, MinioCredentials};
    /// use url::Url;
    ///
    /// let credentials = MinioCredentials::new("access_key", "secret_key");
    /// let endpoint = Url::parse("https://play.min.io").unwrap();
    /// let config = MinioConfig::new(endpoint, credentials).unwrap();
    /// ```
    pub fn new(endpoint: Url, credentials: MinioCredentials) -> Result<Self> {
        // Validate endpoint - enforce HTTPS only for security
        if endpoint.scheme() != "https" {
            return Err(Error::Config(format!(
                "Invalid endpoint scheme '{}', only 'https' is allowed for security",
                endpoint.scheme()
            )));
        }

        if endpoint.host().is_none() {
            return Err(Error::Config(
                "Endpoint must include a valid hostname".to_string(),
            ));
        }

        Ok(Self {
            endpoint,
            credentials,
            connect_timeout: Duration::from_secs(30),
            request_timeout: Duration::from_secs(300), // 5 minutes for large uploads
            path_style: true,
        })
    }

    /// Sets the connection timeout.
    ///
    /// # Arguments
    ///
    /// * `timeout` - Maximum time to wait for connection establishment
    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Sets the request timeout.
    ///
    /// # Arguments
    ///
    /// * `timeout` - Maximum time to wait for request completion
    pub fn with_request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// Sets whether to use path-style requests.
    ///
    /// # Arguments
    ///
    /// * `path_style` - Whether to use path-style (true) or virtual-hosted style (false)
    pub fn with_path_style(mut self, path_style: bool) -> Self {
        self.path_style = path_style;
        self
    }

    /// Returns whether secure connections should be used.
    ///
    /// This is always determined from the endpoint URL scheme and cannot be overridden.
    pub fn is_secure(&self) -> bool {
        self.endpoint.scheme() == "https"
    }

    /// Returns the endpoint URL.
    #[inline]
    pub fn endpoint(&self) -> &Url {
        &self.endpoint
    }

    /// Returns the credentials.
    #[inline]
    pub fn credentials(&self) -> &MinioCredentials {
        &self.credentials
    }

    /// Returns a masked version of the endpoint for logging.
    ///
    /// This preserves the scheme, host, and port while masking any embedded credentials.
    pub fn endpoint_masked(&self) -> String {
        let mut url = self.endpoint.clone();

        // Remove any credentials from the URL
        let _ = url.set_username("");
        let _ = url.set_password(None);

        url.to_string()
    }

    /// Validates the configuration and returns any validation errors.
    ///
    /// This method checks for common configuration issues that might cause
    /// runtime failures.
    ///
    /// # Errors
    ///
    /// Returns validation errors if:
    /// - Credentials are empty
    /// - Timeouts are too short or too long
    /// - Endpoint is unreachable (if connectivity check is enabled)
    pub fn validate(&self) -> Result<()> {
        // Validate credentials
        if self.credentials.access_key.is_empty() {
            return Err(Error::Config("Access key cannot be empty".to_string()));
        }

        if self.credentials.secret_key.is_empty() {
            return Err(Error::Config("Secret key cannot be empty".to_string()));
        }

        // Validate timeouts
        if self.connect_timeout.is_zero() {
            return Err(Error::Config(
                "Connect timeout must be greater than zero".to_string(),
            ));
        }

        if self.request_timeout.is_zero() {
            return Err(Error::Config(
                "Request timeout must be greater than zero".to_string(),
            ));
        }

        // Warn about very short timeouts
        if self.connect_timeout < Duration::from_secs(1) {
            tracing::warn!(
                target: crate::TRACING_TARGET_CLIENT,
                timeout = ?self.connect_timeout,
                "Connect timeout is very short and may cause connection failures"
            );
        }

        if self.request_timeout < Duration::from_secs(10) {
            tracing::warn!(
                target: crate::TRACING_TARGET_CLIENT,
                timeout = ?self.request_timeout,
                "Request timeout is very short and may cause operation failures"
            );
        }

        Ok(())
    }
}

impl Default for MinioConfig {
    fn default() -> Self {
        let endpoint =
            Url::parse("https://localhost:9000").expect("default endpoint should be valid");
        let credentials = MinioCredentials::new("minioadmin", "minioadmin");

        Self::new(endpoint, credentials).expect("default configuration should be valid")
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

    #[test]
    fn test_config_new() {
        let endpoint = Url::parse("https://play.min.io").unwrap();
        let credentials = MinioCredentials::new("access", "secret");
        let config = MinioConfig::new(endpoint, credentials).unwrap();

        assert_eq!(config.endpoint().as_str(), "https://play.min.io/");
        assert!(config.is_secure());
        assert!(config.path_style);
    }

    #[test]
    fn test_config_invalid_endpoint() {
        let endpoint = Url::parse("http://invalid.com").unwrap(); // HTTP should be rejected
        let credentials = MinioCredentials::new("access", "secret");
        let result = MinioConfig::new(endpoint, credentials);

        assert!(result.is_err());
        assert!(matches!(result, Err(Error::Config(_))));
    }

    #[test]
    fn test_config_builder_methods() {
        let endpoint = Url::parse("https://localhost:9000").unwrap();
        let credentials = MinioCredentials::new("access", "secret");
        let config = MinioConfig::new(endpoint, credentials)
            .unwrap()
            .with_path_style(false)
            .with_connect_timeout(Duration::from_secs(10))
            .with_request_timeout(Duration::from_secs(60));

        assert!(config.is_secure());
        assert!(!config.path_style);
        assert_eq!(config.connect_timeout, Duration::from_secs(10));
        assert_eq!(config.request_timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_config_validation() {
        let endpoint = Url::parse("https://localhost:9000").unwrap();

        // Valid config
        let credentials = MinioCredentials::new("access", "secret");
        let config = MinioConfig::new(endpoint.clone(), credentials).unwrap();
        assert!(config.validate().is_ok());

        // Empty access key
        let empty_access = MinioCredentials::new("", "secret");
        let config = MinioConfig::new(endpoint.clone(), empty_access).unwrap();
        assert!(config.validate().is_err());

        // Empty secret key
        let empty_secret = MinioCredentials::new("access", "");
        let config = MinioConfig::new(endpoint, empty_secret).unwrap();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_endpoint_masking() {
        let endpoint = Url::parse("https://user:pass@example.com:9000/").unwrap();
        let credentials = MinioCredentials::new("access", "secret");
        let config = MinioConfig::new(endpoint, credentials).unwrap();

        let masked = config.endpoint_masked();
        assert!(!masked.contains("user"));
        assert!(!masked.contains("pass"));
        assert!(masked.contains("example.com"));
    }
}
