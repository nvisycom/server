//! NATS connection configuration.

use std::time::Duration;

#[cfg(feature = "config")]
use clap::Args;
use serde::{Deserialize, Serialize};

/// Configuration for NATS connections with sensible defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "config", derive(Args))]
pub struct NatsConfig {
    /// NATS server URL (comma-separated for clustering)
    #[cfg_attr(
        feature = "config",
        arg(long, env = "NATS_URL", default_value = "nats://127.0.0.1:4222")
    )]
    pub url: String,

    /// Authentication token (required)
    #[cfg_attr(
        feature = "config",
        arg(id = "nats_token", long = "nats-token", env = "NATS_TOKEN")
    )]
    pub token: String,

    /// Client connection name for debugging and monitoring
    #[cfg_attr(
        feature = "config",
        arg(
            id = "nats_client_name",
            long = "nats-client-name",
            env = "NATS_CLIENT_NAME"
        )
    )]
    pub name: Option<String>,

    /// Maximum time to wait for initial connection in seconds (optional)
    #[cfg_attr(
        feature = "config",
        arg(
            id = "nats_connect_timeout_secs",
            long = "nats-connect-timeout-secs",
            env = "NATS_CONNECT_TIMEOUT_SECS"
        )
    )]
    pub connect_timeout_secs: Option<u64>,

    /// Default timeout for request-reply operations in seconds (optional)
    #[cfg_attr(
        feature = "config",
        arg(
            id = "nats_request_timeout_secs",
            long = "nats-request-timeout-secs",
            env = "NATS_REQUEST_TIMEOUT_SECS"
        )
    )]
    pub request_timeout_secs: Option<u64>,

    /// Maximum number of reconnection attempts (0 = unlimited)
    #[cfg_attr(
        feature = "config",
        arg(
            id = "nats_max_reconnects",
            long = "nats-max-reconnects",
            env = "NATS_MAX_RECONNECTS"
        )
    )]
    pub max_reconnects: Option<usize>,

    /// Delay between reconnection attempts in seconds
    #[cfg_attr(
        feature = "config",
        arg(
            id = "nats_reconnect_delay_secs",
            long = "nats-reconnect-delay-secs",
            env = "NATS_RECONNECT_DELAY_SECS"
        )
    )]
    pub reconnect_delay_secs: Option<u64>,

    /// Interval for sending ping messages in seconds
    #[cfg_attr(
        feature = "config",
        arg(
            id = "nats_ping_interval_secs",
            long = "nats-ping-interval-secs",
            env = "NATS_PING_INTERVAL_SECS"
        )
    )]
    pub ping_interval_secs: Option<u64>,
}

// Default values
const DEFAULT_NAME: &str = "nvisy-nats";
const DEFAULT_MAX_RECONNECTS: usize = 10;
const DEFAULT_RECONNECT_DELAY_SECS: u64 = 2;
const DEFAULT_PING_INTERVAL_SECS: u64 = 30;

impl NatsConfig {
    /// Create a new configuration with a single server URL and token.
    pub fn new(server_url: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            url: server_url.into(),
            token: token.into(),
            name: None,
            connect_timeout_secs: None,
            request_timeout_secs: None,
            max_reconnects: None,
            reconnect_delay_secs: None,
            ping_interval_secs: None,
        }
    }

    /// Returns the client name, using the default if not set.
    #[inline]
    pub fn name(&self) -> &str {
        self.name.as_deref().unwrap_or(DEFAULT_NAME)
    }

    /// Returns the server URLs as a vector (splits comma-separated URLs).
    pub fn servers(&self) -> Vec<&str> {
        self.url.split(',').map(str::trim).collect()
    }

    /// Returns the connection timeout as a Duration, if set.
    #[inline]
    pub fn connect_timeout(&self) -> Option<Duration> {
        self.connect_timeout_secs.map(Duration::from_secs)
    }

    /// Returns the request timeout as a Duration, if set.
    #[inline]
    pub fn request_timeout(&self) -> Option<Duration> {
        self.request_timeout_secs.map(Duration::from_secs)
    }

    /// Returns the reconnect delay as a Duration.
    #[inline]
    pub fn reconnect_delay(&self) -> Duration {
        Duration::from_secs(
            self.reconnect_delay_secs
                .unwrap_or(DEFAULT_RECONNECT_DELAY_SECS),
        )
    }

    /// Returns the ping interval as a Duration.
    #[inline]
    pub fn ping_interval(&self) -> Duration {
        Duration::from_secs(
            self.ping_interval_secs
                .unwrap_or(DEFAULT_PING_INTERVAL_SECS),
        )
    }

    /// Returns the max reconnects as Option (0 means unlimited).
    #[inline]
    pub fn max_reconnects_option(&self) -> Option<usize> {
        let max = self.max_reconnects.unwrap_or(DEFAULT_MAX_RECONNECTS);
        if max == 0 { None } else { Some(max) }
    }

    /// Set server URL(s).
    #[must_use]
    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = url.into();
        self
    }

    /// Set the authentication token.
    #[must_use]
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.token = token.into();
        self
    }

    /// Set the client connection name.
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the connection timeout in seconds.
    #[must_use]
    pub fn with_connect_timeout_secs(mut self, secs: u64) -> Self {
        self.connect_timeout_secs = Some(secs);
        self
    }

    /// Set the request timeout in seconds.
    #[must_use]
    pub fn with_request_timeout_secs(mut self, secs: u64) -> Self {
        self.request_timeout_secs = Some(secs);
        self
    }

    /// Set maximum reconnection attempts (0 for unlimited).
    #[must_use]
    pub fn with_max_reconnects(mut self, max_reconnects: usize) -> Self {
        self.max_reconnects = Some(max_reconnects);
        self
    }

    /// Set the delay between reconnection attempts in seconds.
    #[must_use]
    pub fn with_reconnect_delay_secs(mut self, secs: u64) -> Self {
        self.reconnect_delay_secs = Some(secs);
        self
    }

    /// Set the ping interval in seconds.
    #[must_use]
    pub fn with_ping_interval_secs(mut self, secs: u64) -> Self {
        self.ping_interval_secs = Some(secs);
        self
    }

    /// Validate the configuration and return any issues.
    pub fn validate(&self) -> Result<(), String> {
        let servers = self.servers();

        if servers.is_empty() {
            return Err("At least one server URL must be provided".to_string());
        }

        for server in servers {
            if server.is_empty() {
                return Err("Server URL cannot be empty".to_string());
            }
            if !server.starts_with("nats://") {
                return Err(format!("Invalid server URL format: {}", server));
            }
        }

        if self.token.is_empty() {
            return Err("Token cannot be empty".to_string());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_config() {
        let config = NatsConfig::new("nats://localhost:4222", "my-token");
        assert_eq!(config.servers(), vec!["nats://localhost:4222"]);
        assert_eq!(config.token, "my-token");
        assert_eq!(config.name(), "nvisy-nats");
        assert_eq!(config.connect_timeout(), None);
        assert_eq!(config.request_timeout(), None);
        assert_eq!(config.max_reconnects_option(), Some(10));
    }

    #[test]
    fn test_config_builder() {
        let config = NatsConfig::new("nats://localhost:4222", "my-token")
            .with_name("test-client")
            .with_connect_timeout_secs(5)
            .with_request_timeout_secs(15)
            .with_max_reconnects(5);

        assert_eq!(config.servers(), vec!["nats://localhost:4222"]);
        assert_eq!(config.name(), "test-client");
        assert_eq!(config.connect_timeout(), Some(Duration::from_secs(5)));
        assert_eq!(config.request_timeout(), Some(Duration::from_secs(15)));
        assert_eq!(config.max_reconnects_option(), Some(5));
    }

    #[test]
    fn test_unlimited_reconnects() {
        let config = NatsConfig::new("nats://localhost:4222", "token").with_max_reconnects(0);
        assert_eq!(config.max_reconnects_option(), None); // Unlimited
    }

    #[test]
    fn test_config_validation() {
        let valid_config = NatsConfig::new("nats://localhost:4222", "my-token");
        assert!(valid_config.validate().is_ok());

        let empty_url = NatsConfig::new("", "my-token");
        assert!(empty_url.validate().is_err());

        let invalid_url = NatsConfig::new("invalid-url", "my-token");
        assert!(invalid_url.validate().is_err());

        let empty_token = NatsConfig::new("nats://localhost:4222", "");
        assert!(empty_token.validate().is_err());
    }

    #[test]
    fn test_multiple_servers() {
        let config = NatsConfig::new(
            "nats://localhost:4222, nats://localhost:4223, nats://localhost:4224",
            "token",
        );

        assert_eq!(
            config.servers(),
            vec![
                "nats://localhost:4222",
                "nats://localhost:4223",
                "nats://localhost:4224"
            ]
        );
    }

    #[test]
    fn test_duration_helpers() {
        let config = NatsConfig::new("nats://localhost:4222", "token")
            .with_connect_timeout_secs(10)
            .with_request_timeout_secs(30);
        assert_eq!(config.connect_timeout(), Some(Duration::from_secs(10)));
        assert_eq!(config.request_timeout(), Some(Duration::from_secs(30)));
        assert_eq!(config.reconnect_delay(), Duration::from_secs(2));
        assert_eq!(config.ping_interval(), Duration::from_secs(30));
    }

    #[test]
    fn test_default_values() {
        let config = NatsConfig::new("nats://localhost:4222", "token");
        assert_eq!(config.name(), DEFAULT_NAME);
        assert_eq!(
            config.reconnect_delay(),
            Duration::from_secs(DEFAULT_RECONNECT_DELAY_SECS)
        );
        assert_eq!(
            config.ping_interval(),
            Duration::from_secs(DEFAULT_PING_INTERVAL_SECS)
        );
        assert_eq!(config.max_reconnects_option(), Some(DEFAULT_MAX_RECONNECTS));
    }
}
