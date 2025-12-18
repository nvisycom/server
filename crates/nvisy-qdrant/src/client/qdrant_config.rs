//! Qdrant client configuration.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::{QdrantError, QdrantResult};

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

    /// Compression settings
    pub compression: CompressionConfig,

    /// Retry configuration
    pub retry: RetryConfig,
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
    pub fn new(url: impl Into<String>) -> QdrantResult<Self> {
        let url = url.into();

        // Validate URL format
        if url.is_empty() {
            return Err(QdrantError::invalid_config("URL cannot be empty"));
        }

        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(QdrantError::invalid_config(
                "URL must start with http:// or https://",
            ));
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
            compression: CompressionConfig::default(),
            retry: RetryConfig::default(),
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

    /// Set compression configuration.
    pub fn compression(mut self, compression: CompressionConfig) -> Self {
        self.compression = compression;
        self
    }

    /// Set retry configuration.
    pub fn retry(mut self, retry: RetryConfig) -> Self {
        self.retry = retry;
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
    pub fn validate(&self) -> QdrantResult<()> {
        // Validate URL
        if self.url.is_empty() {
            return Err(QdrantError::invalid_config("URL cannot be empty"));
        }

        // Validate timeouts
        if self.connect_timeout.is_zero() {
            return Err(QdrantError::invalid_config(
                "Connect timeout must be greater than zero",
            ));
        }

        if self.timeout.is_zero() {
            return Err(QdrantError::invalid_config(
                "Request timeout must be greater than zero",
            ));
        }

        // Validate pool size
        if let Some(pool_size) = self.pool_size {
            if pool_size == 0 {
                return Err(QdrantError::invalid_config(
                    "Pool size must be greater than zero",
                ));
            }
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
    pub fn validate(&self) -> QdrantResult<()> {
        // Validate client certificate and key are both present or both absent
        match (&self.client_certificate_pem, &self.client_private_key_pem) {
            (Some(_), None) => {
                return Err(QdrantError::invalid_config(
                    "Client certificate provided but private key is missing",
                ));
            }
            (None, Some(_)) => {
                return Err(QdrantError::invalid_config(
                    "Client private key provided but certificate is missing",
                ));
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

/// Compression configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompressionConfig {
    /// Enable gzip compression for requests
    pub gzip: bool,

    /// Enable brotli compression for requests
    pub brotli: bool,

    /// Enable deflate compression for requests
    pub deflate: bool,
}

impl CompressionConfig {
    /// Create compression configuration with all methods disabled.
    pub fn none() -> Self {
        Self {
            gzip: false,
            brotli: false,
            deflate: false,
        }
    }

    /// Create compression configuration with all methods enabled.
    pub fn all() -> Self {
        Self {
            gzip: true,
            brotli: true,
            deflate: true,
        }
    }

    /// Enable only gzip compression.
    pub fn gzip_only() -> Self {
        Self {
            gzip: true,
            brotli: false,
            deflate: false,
        }
    }
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self::gzip_only()
    }
}

/// Retry configuration for failed requests.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: u32,

    /// Initial retry delay
    pub initial_delay: Duration,

    /// Maximum retry delay
    pub max_delay: Duration,

    /// Backoff multiplier (exponential backoff)
    pub backoff_multiplier: f64,

    /// Jitter factor to avoid thundering herd (0.0 to 1.0)
    pub jitter: f64,
}

impl RetryConfig {
    /// Create a new retry configuration.
    pub fn new(max_attempts: u32) -> Self {
        Self {
            max_attempts,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            jitter: 0.1,
        }
    }

    /// Disable retries.
    pub fn none() -> Self {
        Self {
            max_attempts: 0,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            jitter: 0.1,
        }
    }

    /// Set the initial delay between retries.
    pub fn initial_delay(mut self, delay: Duration) -> Self {
        self.initial_delay = delay;
        self
    }

    /// Set the maximum delay between retries.
    pub fn max_delay(mut self, delay: Duration) -> Self {
        self.max_delay = delay;
        self
    }

    /// Set the backoff multiplier for exponential backoff.
    pub fn backoff_multiplier(mut self, multiplier: f64) -> Self {
        self.backoff_multiplier = multiplier;
        self
    }

    /// Set the jitter factor to add randomness to delays.
    pub fn jitter(mut self, jitter: f64) -> Self {
        self.jitter = jitter.clamp(0.0, 1.0);
        self
    }

    /// Calculate the delay for a given attempt number.
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        if attempt == 0 || self.max_attempts == 0 {
            return Duration::from_millis(0);
        }

        let delay_ms = self.initial_delay.as_millis() as f64
            * self.backoff_multiplier.powi(attempt as i32 - 1);

        let delay = Duration::from_millis(delay_ms as u64);
        let capped_delay = delay.min(self.max_delay);

        // Add jitter
        if self.jitter > 0.0 {
            let jitter_range = capped_delay.as_millis() as f64 * self.jitter;
            let jitter_ms = fastrand::f64() * jitter_range;
            let final_delay_ms = capped_delay.as_millis() as f64 + jitter_ms;
            Duration::from_millis(final_delay_ms as u64)
        } else {
            capped_delay
        }
    }
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self::new(3) // 3 retry attempts by default
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
    fn test_compression_config() {
        let none = CompressionConfig::none();
        assert!(!none.gzip && !none.brotli && !none.deflate);

        let all = CompressionConfig::all();
        assert!(all.gzip && all.brotli && all.deflate);

        let gzip_only = CompressionConfig::gzip_only();
        assert!(gzip_only.gzip && !gzip_only.brotli && !gzip_only.deflate);
    }

    #[test]
    fn test_retry_config() {
        let retry = RetryConfig::new(5)
            .initial_delay(Duration::from_millis(200))
            .backoff_multiplier(1.5)
            .jitter(0.2);

        assert_eq!(retry.max_attempts, 5);
        assert_eq!(retry.initial_delay, Duration::from_millis(200));
        assert_eq!(retry.backoff_multiplier, 1.5);
        assert_eq!(retry.jitter, 0.2);

        // Test delay calculation
        let delay1 = retry.calculate_delay(1);
        let delay2 = retry.calculate_delay(2);
        assert!(delay2 > delay1); // Should increase with exponential backoff
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
