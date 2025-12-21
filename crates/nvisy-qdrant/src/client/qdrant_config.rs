//! Qdrant client configuration.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Configuration for Qdrant client connections.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QdrantConfig {
    /// Qdrant server URL (e.g., "http://localhost:6334")
    pub url: String,

    /// API key for authentication (optional)
    pub api_key: Option<String>,

    /// Connection timeout
    pub connect_timeout: Duration,

    /// Request timeout
    pub timeout: Duration,

    /// Keep-alive timeout
    pub keep_alive_timeout: Option<Duration>,

    /// Maximum number of concurrent connections
    pub pool_size: Option<usize>,

    /// Enable keep-alive
    pub keep_alive: bool,

    /// User agent string
    pub user_agent: Option<String>,

    /// TLS configuration
    pub tls: Option<QdrantTlsConfig>,
}

impl QdrantConfig {
    /// Create a new Qdrant configuration with the given URL.
    ///
    /// # Arguments
    ///
    /// * `url` - The Qdrant server URL (e.g., "http://localhost:6334")
    ///
    /// # Errors
    ///
    /// Returns an error if the URL is invalid.
    pub fn new(url: impl Into<String>) -> Result<Self> {
        let url = url.into();

        // Validate URL format
        if url.is_empty() {
            return Err(Error::configuration().with_message("URL cannot be empty"));
        }

        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(
                Error::configuration().with_message("URL must start with http:// or https://")
            );
        }

        Ok(Self {
            url,
            api_key: None,
            connect_timeout: Duration::from_secs(10),
            timeout: Duration::from_secs(30),
            keep_alive_timeout: Some(Duration::from_secs(90)),
            pool_size: Some(10),
            keep_alive: true,
            user_agent: Some(format!(
                "nvisy-qdrant/{} ({})",
                env!("CARGO_PKG_VERSION"),
                std::env::consts::OS
            )),
            tls: None,
        })
    }

    /// Set the API key for authentication.
    pub fn api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Set the connection timeout.
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Set the request timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set the keep-alive timeout.
    pub fn keep_alive_timeout(mut self, timeout: Duration) -> Self {
        self.keep_alive_timeout = Some(timeout);
        self
    }

    /// Set the connection pool size.
    pub fn pool_size(mut self, size: usize) -> Self {
        self.pool_size = Some(size);
        self
    }

    /// Enable or disable keep-alive.
    pub fn keep_alive(mut self, enable: bool) -> Self {
        self.keep_alive = enable;
        self
    }

    /// Set a custom user agent string.
    pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }

    /// Enable TLS with the given configuration.
    pub fn tls(mut self, tls: QdrantTlsConfig) -> Self {
        self.tls = Some(tls);
        self
    }

    /// Enable TLS with default settings.
    pub fn enable_tls(mut self) -> Self {
        self.tls = Some(QdrantTlsConfig::default());
        self
    }

    /// Check if TLS is enabled.
    pub fn is_tls_enabled(&self) -> bool {
        self.tls.is_some() || self.url.starts_with("https://")
    }

    /// Get the base URL without path.
    pub fn base_url(&self) -> &str {
        &self.url
    }

    /// Validate the configuration.
    pub fn validate(&self) -> Result<()> {
        // Validate URL
        if self.url.is_empty() {
            return Err(Error::configuration().with_message("URL cannot be empty"));
        }

        // Validate timeouts
        if self.connect_timeout.is_zero() {
            return Err(
                Error::configuration().with_message("Connect timeout must be greater than zero")
            );
        }

        if self.timeout.is_zero() {
            return Err(
                Error::configuration().with_message("Request timeout must be greater than zero")
            );
        }

        // Validate pool size
        if let Some(pool_size) = self.pool_size
            && pool_size == 0
        {
            return Err(Error::configuration().with_message("Pool size must be greater than zero"));
        }

        // Validate TLS configuration if present
        if let Some(ref tls) = self.tls {
            tls.validate()?;
        }

        Ok(())
    }
}

impl Default for QdrantConfig {
    fn default() -> Self {
        Self::new("http://localhost:6334").expect("Default URL should be valid")
    }
}

/// TLS configuration for Qdrant connections.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QdrantTlsConfig {
    /// Accept invalid certificates (for development only)
    pub accept_invalid_certs: bool,

    /// Accept invalid hostnames (for development only)
    pub accept_invalid_hostnames: bool,

    /// CA certificate in PEM format
    pub ca_certificate_pem: Option<String>,

    /// Client certificate in PEM format
    pub client_certificate_pem: Option<String>,

    /// Client private key in PEM format
    pub client_private_key_pem: Option<String>,
}

impl QdrantTlsConfig {
    /// Create a new TLS configuration with secure defaults.
    pub fn new() -> Self {
        Self {
            accept_invalid_certs: false,
            accept_invalid_hostnames: false,
            ca_certificate_pem: None,
            client_certificate_pem: None,
            client_private_key_pem: None,
        }
    }

    /// Create a TLS configuration for development (accepts invalid certs).
    ///
    /// **Warning**: This is insecure and should only be used in development.
    pub fn insecure() -> Self {
        Self {
            accept_invalid_certs: true,
            accept_invalid_hostnames: true,
            ca_certificate_pem: None,
            client_certificate_pem: None,
            client_private_key_pem: None,
        }
    }

    /// Set CA certificate in PEM format.
    pub fn ca_certificate_pem(mut self, ca_cert: impl Into<String>) -> Self {
        self.ca_certificate_pem = Some(ca_cert.into());
        self
    }

    /// Set client certificate and private key in PEM format.
    pub fn client_certificate_pem(
        mut self,
        cert: impl Into<String>,
        key: impl Into<String>,
    ) -> Self {
        self.client_certificate_pem = Some(cert.into());
        self.client_private_key_pem = Some(key.into());
        self
    }

    /// Accept invalid certificates (for development only).
    pub fn accept_invalid_certs(mut self, accept: bool) -> Self {
        self.accept_invalid_certs = accept;
        self
    }

    /// Accept invalid hostnames (for development only).
    pub fn accept_invalid_hostnames(mut self, accept: bool) -> Self {
        self.accept_invalid_hostnames = accept;
        self
    }

    /// Validate the TLS configuration.
    pub fn validate(&self) -> Result<()> {
        // Validate client certificate and key are both present or both absent
        match (&self.client_certificate_pem, &self.client_private_key_pem) {
            (Some(_), None) => {
                return Err(Error::configuration()
                    .with_message("Client certificate provided but private key is missing"));
            }
            (None, Some(_)) => {
                return Err(Error::configuration()
                    .with_message("Client private key provided but certificate is missing"));
            }
            _ => {}
        }

        Ok(())
    }

    /// Check if client certificate authentication is configured.
    pub fn has_client_cert(&self) -> bool {
        self.client_certificate_pem.is_some() && self.client_private_key_pem.is_some()
    }
}

impl Default for QdrantTlsConfig {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_creation() {
        let config = QdrantConfig::new("http://localhost:6334").unwrap();
        assert_eq!(config.url, "http://localhost:6334");
        assert!(config.api_key.is_none());
        assert_eq!(config.connect_timeout, Duration::from_secs(10));
        assert_eq!(config.timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_config_invalid_url() {
        assert!(QdrantConfig::new("").is_err());
        assert!(QdrantConfig::new("not-a-url").is_err());
        assert!(QdrantConfig::new("ftp://localhost").is_err());
    }

    #[test]
    fn test_config_fluent_api() {
        let config = QdrantConfig::new("https://example.com:6334")
            .unwrap()
            .api_key("secret-key")
            .timeout(Duration::from_secs(60))
            .pool_size(20)
            .enable_tls();

        assert_eq!(config.api_key, Some("secret-key".to_string()));
        assert_eq!(config.timeout, Duration::from_secs(60));
        assert_eq!(config.pool_size, Some(20));
        assert!(config.is_tls_enabled());
    }

    #[test]
    fn test_tls_config() {
        let tls = QdrantTlsConfig::new()
            .ca_certificate_pem("ca-cert")
            .client_certificate_pem("client-cert", "client-key");

        assert!(tls.has_client_cert());
        assert_eq!(tls.ca_certificate_pem, Some("ca-cert".to_string()));
        assert_eq!(tls.client_certificate_pem, Some("client-cert".to_string()));
        assert_eq!(tls.client_private_key_pem, Some("client-key".to_string()));
    }

    #[test]
    fn test_tls_config_validation() {
        let invalid_tls = QdrantTlsConfig {
            client_certificate_pem: Some("cert".to_string()),
            client_private_key_pem: None,
            ..Default::default()
        };
        assert!(invalid_tls.validate().is_err());

        let valid_tls = QdrantTlsConfig::new();
        assert!(valid_tls.validate().is_ok());
    }

    #[test]
    fn test_config_validation() {
        let valid_config = QdrantConfig::default();
        assert!(valid_config.validate().is_ok());

        let mut invalid_config = QdrantConfig::default();
        invalid_config.connect_timeout = Duration::from_secs(0);
        assert!(invalid_config.validate().is_err());
    }
}
