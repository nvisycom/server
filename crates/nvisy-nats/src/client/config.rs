//! NATS connection configuration and credentials.

use std::time::Duration;

/// Configuration for NATS connections
#[derive(Debug, Clone)]
pub struct NatsConfig {
    /// NATS server URL(s)
    pub servers: Vec<String>,
    /// Connection name for debugging
    pub name: String,
    /// Connection timeout
    pub connect_timeout: Duration,
    /// Request timeout for RPC calls
    pub request_timeout: Duration,
    /// Maximum reconnection attempts
    pub max_reconnects: Option<usize>,
    /// Reconnection delay
    pub reconnect_delay: Duration,
    /// Ping interval for keep-alive
    pub ping_interval: Duration,
    /// Authentication credentials
    pub credentials: Option<NatsCredentials>,
    /// TLS configuration
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
    /// Create a new configuration with the given server URL
    pub fn new(server_url: impl Into<String>) -> Self {
        Self {
            servers: vec![server_url.into()],
            ..Default::default()
        }
    }

    /// Add multiple server URLs for clustering
    pub fn with_servers(mut self, servers: Vec<String>) -> Self {
        self.servers = servers;
        self
    }

    /// Set connection name
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Set connection timeout
    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Set request timeout
    pub fn with_request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// Set authentication credentials
    pub fn with_credentials(mut self, credentials: NatsCredentials) -> Self {
        self.credentials = Some(credentials);
        self
    }

    /// Enable TLS
    pub fn with_tls(mut self, tls_config: NatsTlsConfig) -> Self {
        self.tls = Some(tls_config);
        self
    }

    /// Create configuration from environment variables
    pub fn from_env() -> Self {
        let mut config = Self::default();

        if let Ok(servers) = std::env::var("NATS_SERVERS") {
            config.servers = servers.split(',').map(|s| s.trim().to_string()).collect();
        } else if let Ok(url) = std::env::var("NATS_URL") {
            config.servers = vec![url];
        }

        if let Ok(name) = std::env::var("NATS_CLIENT_NAME") {
            config.name = name;
        }

        if let Ok(timeout_str) = std::env::var("NATS_CONNECT_TIMEOUT")
            && let Ok(timeout_secs) = timeout_str.parse::<u64>()
        {
            config.connect_timeout = Duration::from_secs(timeout_secs);
        }

        // Load credentials from environment
        if let (Ok(user), Ok(pass)) = (std::env::var("NATS_USER"), std::env::var("NATS_PASS")) {
            config.credentials = Some(NatsCredentials::UserPassword { user, pass });
        } else if let Ok(token) = std::env::var("NATS_TOKEN") {
            config.credentials = Some(NatsCredentials::Token { token });
        } else if let Ok(creds_file) = std::env::var("NATS_CREDS_FILE") {
            config.credentials = Some(NatsCredentials::CredsFile { path: creds_file });
        }

        config
    }
}

/// NATS authentication credentials
#[derive(Debug, Clone)]
pub enum NatsCredentials {
    /// Username and password
    UserPassword { user: String, pass: String },
    /// JWT token
    Token { token: String },
    /// Credentials file path
    CredsFile { path: String },
    /// NKey seed
    NKey { seed: String },
}

/// TLS configuration for NATS
#[derive(Debug, Clone)]
pub struct NatsTlsConfig {
    /// Enable TLS
    pub enabled: bool,
    /// Skip certificate verification (insecure)
    pub insecure: bool,
    /// Custom CA certificates path
    pub ca_file: Option<String>,
    /// Client certificate for mutual TLS
    pub cert_file: Option<String>,
    /// Client private key for mutual TLS
    pub key_file: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() {
        let config = NatsConfig::new("nats://localhost:4222")
            .with_name("test-client")
            .with_connect_timeout(Duration::from_secs(5))
            .with_request_timeout(Duration::from_secs(10));

        assert_eq!(config.servers, vec!["nats://localhost:4222"]);
        assert_eq!(config.name, "test-client");
        assert_eq!(config.connect_timeout, Duration::from_secs(5));
        assert_eq!(config.request_timeout, Duration::from_secs(10));
    }

    #[test]
    fn test_credentials_variants() {
        let user_pass = NatsCredentials::UserPassword {
            user: "testuser".to_string(),
            pass: "testpass".to_string(),
        };

        let token = NatsCredentials::Token {
            token: "jwt_token_here".to_string(),
        };

        // Verify the variants exist and can be created
        match user_pass {
            NatsCredentials::UserPassword { user, pass } => {
                assert_eq!(user, "testuser");
                assert_eq!(pass, "testpass");
            }
            _ => panic!("Wrong credential type"),
        }

        match token {
            NatsCredentials::Token { token } => {
                assert_eq!(token, "jwt_token_here");
            }
            _ => panic!("Wrong credential type"),
        }
    }
}
