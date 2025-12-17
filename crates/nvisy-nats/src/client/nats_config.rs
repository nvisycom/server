//! NATS connection configuration.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::credentials::NatsCredentials;

/// Configuration for NATS connections with sensible defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NatsConfig {
    /// NATS server URLs for connection (supports clustering)
    pub servers: Vec<String>,
    /// Client connection name for debugging and monitoring
    pub name: String,
    /// Maximum time to wait for initial connection
    pub connect_timeout: Duration,
    /// Default timeout for request-reply operations
    pub request_timeout: Duration,
    /// Maximum number of reconnection attempts (None = unlimited)
    pub max_reconnects: Option<usize>,
    /// Delay between reconnection attempts
    pub reconnect_delay: Duration,
    /// Interval for sending ping messages to maintain connection
    pub ping_interval: Duration,
    /// Authentication credentials (optional)
    pub credentials: Option<NatsCredentials>,
    /// TLS configuration (optional)
    pub tls: Option<NatsTlsConfig>,
}

impl Default for NatsConfig {
    fn default() -> Self {
        Self {
            servers: vec!["nats://127.0.0.1:4222".to_string()],
            name: "nvisy-nats".to_string(),
            connect_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(30),
            max_reconnects: Some(10),
            reconnect_delay: Duration::from_secs(2),
            ping_interval: Duration::from_secs(30),
            credentials: None,
            tls: None,
        }
    }
}

impl NatsConfig {
    /// Create a new configuration with a single server URL.
    pub fn new(server_url: impl Into<String>) -> Self {
        Self {
            servers: vec![server_url.into()],
            ..Default::default()
        }
    }

    /// Set multiple server URLs for clustering support.
    #[must_use]
    pub fn with_servers(mut self, servers: Vec<String>) -> Self {
        self.servers = servers;
        self
    }

    /// Add a single server URL to the existing list.
    #[must_use]
    pub fn add_server(mut self, server_url: impl Into<String>) -> Self {
        self.servers.push(server_url.into());
        self
    }

    /// Set the client connection name.
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Set the connection timeout.
    #[must_use]
    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Set the request timeout for request-reply operations.
    #[must_use]
    pub fn with_request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// Set maximum reconnection attempts (None for unlimited).
    #[must_use]
    pub fn with_max_reconnects(mut self, max_reconnects: Option<usize>) -> Self {
        self.max_reconnects = max_reconnects;
        self
    }

    /// Set the delay between reconnection attempts.
    #[must_use]
    pub fn with_reconnect_delay(mut self, delay: Duration) -> Self {
        self.reconnect_delay = delay;
        self
    }

    /// Set the ping interval for connection keep-alive.
    pub fn with_ping_interval(mut self, interval: Duration) -> Self {
        self.ping_interval = interval;
        self
    }

    /// Set authentication credentials.
    pub fn with_credentials(mut self, credentials: NatsCredentials) -> Self {
        self.credentials = Some(credentials);
        self
    }

    /// Enable TLS with the provided configuration.
    pub fn with_tls(mut self, tls_config: NatsTlsConfig) -> Self {
        self.tls = Some(tls_config);
        self
    }

    /// Create a production-ready configuration with extended timeouts.
    pub fn production() -> Self {
        Self {
            connect_timeout: Duration::from_secs(30),
            request_timeout: Duration::from_secs(60),
            max_reconnects: None, // Unlimited reconnects in production
            reconnect_delay: Duration::from_secs(5),
            ping_interval: Duration::from_secs(60),
            ..Default::default()
        }
    }

    /// Create a development configuration with shorter timeouts.
    pub fn development() -> Self {
        Self {
            connect_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_secs(10),
            max_reconnects: Some(3),
            reconnect_delay: Duration::from_secs(1),
            ping_interval: Duration::from_secs(15),
            ..Default::default()
        }
    }

    /// Validate the configuration and return any issues.
    pub fn validate(&self) -> Result<(), String> {
        if self.servers.is_empty() {
            return Err("At least one server URL must be provided".to_string());
        }

        for server in &self.servers {
            if server.is_empty() {
                return Err("Server URL cannot be empty".to_string());
            }
            if !server.starts_with("nats://") && !server.starts_with("tls://") {
                return Err(format!("Invalid server URL format: {}", server));
            }
        }

        if self.name.is_empty() {
            return Err("Client name cannot be empty".to_string());
        }

        if self.connect_timeout.is_zero() {
            return Err("Connect timeout must be greater than zero".to_string());
        }

        if self.request_timeout.is_zero() {
            return Err("Request timeout must be greater than zero".to_string());
        }

        Ok(())
    }
}

/// TLS configuration for secure NATS connections.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NatsTlsConfig {
    /// Whether TLS is enabled
    pub enabled: bool,
    /// Skip certificate verification (WARNING: insecure, only for testing)
    pub insecure: bool,
    /// Path to custom CA certificates file
    pub ca_file: Option<String>,
    /// Path to client certificate file (for mutual TLS)
    pub cert_file: Option<String>,
    /// Path to client private key file (for mutual TLS)
    pub key_file: Option<String>,
}

impl Default for NatsTlsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            insecure: false,
            ca_file: None,
            cert_file: None,
            key_file: None,
        }
    }
}

impl NatsTlsConfig {
    /// Create a new TLS configuration with secure defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable insecure mode (skip certificate verification).
    /// WARNING: This should only be used for testing.
    pub fn insecure(mut self) -> Self {
        self.insecure = true;
        self
    }

    /// Set custom CA certificates file.
    pub fn with_ca_file(mut self, ca_file: impl Into<String>) -> Self {
        self.ca_file = Some(ca_file.into());
        self
    }

    /// Configure mutual TLS with client certificate and key.
    pub fn with_client_cert(
        mut self,
        cert_file: impl Into<String>,
        key_file: impl Into<String>,
    ) -> Self {
        self.cert_file = Some(cert_file.into());
        self.key_file = Some(key_file.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = NatsConfig::default();
        assert_eq!(config.servers, vec!["nats://127.0.0.1:4222"]);
        assert_eq!(config.name, "nvisy-nats");
        assert_eq!(config.connect_timeout, Duration::from_secs(10));
        assert_eq!(config.request_timeout, Duration::from_secs(30));
        assert_eq!(config.max_reconnects, Some(10));
        assert!(config.credentials.is_none());
        assert!(config.tls.is_none());
    }

    #[test]
    fn test_config_builder() {
        let config = NatsConfig::new("nats://localhost:4222")
            .with_name("test-client")
            .with_connect_timeout(Duration::from_secs(5))
            .with_request_timeout(Duration::from_secs(15))
            .with_max_reconnects(Some(5));

        assert_eq!(config.servers, vec!["nats://localhost:4222"]);
        assert_eq!(config.name, "test-client");
        assert_eq!(config.connect_timeout, Duration::from_secs(5));
        assert_eq!(config.request_timeout, Duration::from_secs(15));
        assert_eq!(config.max_reconnects, Some(5));
    }

    #[test]
    fn test_production_config() {
        let config = NatsConfig::production();
        assert_eq!(config.connect_timeout, Duration::from_secs(30));
        assert_eq!(config.request_timeout, Duration::from_secs(60));
        assert_eq!(config.max_reconnects, None);
        assert_eq!(config.reconnect_delay, Duration::from_secs(5));
        assert_eq!(config.ping_interval, Duration::from_secs(60));
    }

    #[test]
    fn test_development_config() {
        let config = NatsConfig::development();
        assert_eq!(config.connect_timeout, Duration::from_secs(5));
        assert_eq!(config.request_timeout, Duration::from_secs(10));
        assert_eq!(config.max_reconnects, Some(3));
        assert_eq!(config.reconnect_delay, Duration::from_secs(1));
        assert_eq!(config.ping_interval, Duration::from_secs(15));
    }

    #[test]
    fn test_config_validation() {
        let valid_config = NatsConfig::default();
        assert!(valid_config.validate().is_ok());

        let empty_servers = NatsConfig {
            servers: vec![],
            ..Default::default()
        };
        assert!(empty_servers.validate().is_err());

        let invalid_url = NatsConfig {
            servers: vec!["invalid-url".to_string()],
            ..Default::default()
        };
        assert!(invalid_url.validate().is_err());

        let empty_name = NatsConfig {
            name: String::new(),
            ..Default::default()
        };
        assert!(empty_name.validate().is_err());
    }

    #[test]
    fn test_credentials() {
        let user_pass = NatsCredentials::user_password("testuser", "testpass");
        match user_pass {
            NatsCredentials::UserPassword { user, pass } => {
                assert_eq!(user, "testuser");
                assert_eq!(pass, "testpass");
            }
            _ => panic!("Expected UserPassword credentials"),
        }

        let token = NatsCredentials::token("jwt_token_here");
        match token {
            NatsCredentials::Token { token } => {
                assert_eq!(token, "jwt_token_here");
            }
            _ => panic!("Expected Token credentials"),
        }

        let creds_file = NatsCredentials::creds_file("/path/to/creds.txt");
        match creds_file {
            NatsCredentials::CredsFile { path } => {
                assert_eq!(path, "/path/to/creds.txt");
            }
            _ => panic!("Expected CredsFile credentials"),
        }
    }

    #[test]
    fn test_tls_config() {
        let tls = NatsTlsConfig::new()
            .insecure()
            .with_ca_file("/path/to/ca.pem")
            .with_client_cert("/path/to/cert.pem", "/path/to/key.pem");

        assert!(tls.enabled);
        assert!(tls.insecure);
        assert_eq!(tls.ca_file, Some("/path/to/ca.pem".to_string()));
        assert_eq!(tls.cert_file, Some("/path/to/cert.pem".to_string()));
        assert_eq!(tls.key_file, Some("/path/to/key.pem".to_string()));
    }

    #[test]
    fn test_add_server() {
        let config = NatsConfig::new("nats://localhost:4222")
            .add_server("nats://localhost:4223")
            .add_server("nats://localhost:4224");

        assert_eq!(
            config.servers,
            vec![
                "nats://localhost:4222",
                "nats://localhost:4223",
                "nats://localhost:4224"
            ]
        );
    }
}
