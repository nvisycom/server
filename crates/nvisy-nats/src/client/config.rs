//! NATS connection configuration.

use std::time::Duration;

/// Configuration for NATS connections with sensible defaults.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "cli", derive(clap::Args))]
pub struct NatsConfig {
    /// NATS server URL (comma-separated for clustering)
    #[cfg_attr(feature = "cli", arg(long = "nats-url", env = "NATS_URL"))]
    pub nats_url: String,

    /// Authentication token
    #[cfg_attr(feature = "cli", arg(long = "nats-token", env = "NATS_TOKEN"))]
    pub nats_token: String,

    /// Client connection name for debugging and monitoring
    #[cfg_attr(
        feature = "cli",
        arg(long = "nats-client-name", env = "NATS_CLIENT_NAME")
    )]
    pub nats_client_name: Option<String>,

    /// Connection timeout (optional).
    #[cfg_attr(
        feature = "cli",
        arg(
            long = "nats-connect-timeout",
            env = "NATS_CONNECT_TIMEOUT",
            value_parser = humantime::parse_duration,
        )
    )]
    pub nats_connect_timeout: Option<Duration>,

    /// Request timeout (optional).
    #[cfg_attr(
        feature = "cli",
        arg(
            long = "nats-request-timeout",
            env = "NATS_REQUEST_TIMEOUT",
            value_parser = humantime::parse_duration,
        )
    )]
    pub nats_request_timeout: Option<Duration>,

    /// Maximum number of reconnection attempts (0 = unlimited)
    #[cfg_attr(
        feature = "cli",
        arg(long = "nats-max-reconnects", env = "NATS_MAX_RECONNECTS")
    )]
    pub nats_max_reconnects: Option<usize>,
}

// Default values
const DEFAULT_NAME: &str = "nvisy-nats";
const DEFAULT_MAX_RECONNECTS: usize = 10;
const DEFAULT_RECONNECT_DELAY: Duration = Duration::from_secs(2);
const DEFAULT_PING_INTERVAL: Duration = Duration::from_secs(30);

impl NatsConfig {
    /// Create a new configuration with a single server URL and token.
    pub fn new(server_url: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            nats_url: server_url.into(),
            nats_token: token.into(),
            nats_client_name: None,
            nats_connect_timeout: None,
            nats_request_timeout: None,
            nats_max_reconnects: None,
        }
    }

    /// Returns the client name, using the default if not set.
    #[inline]
    pub fn name(&self) -> &str {
        self.nats_client_name.as_deref().unwrap_or(DEFAULT_NAME)
    }

    /// Returns the reconnect delay as a Duration.
    #[inline]
    pub fn reconnect_delay(&self) -> Duration {
        DEFAULT_RECONNECT_DELAY
    }

    /// Returns the ping interval as a Duration.
    #[inline]
    pub fn ping_interval(&self) -> Duration {
        DEFAULT_PING_INTERVAL
    }

    /// Returns the max reconnects as Option (0 means unlimited).
    #[inline]
    pub fn max_reconnects_option(&self) -> Option<usize> {
        let max = self.nats_max_reconnects.unwrap_or(DEFAULT_MAX_RECONNECTS);
        if max == 0 { None } else { Some(max) }
    }

    /// Set the request-reply timeout.
    #[must_use]
    pub fn with_request_timeout(mut self, timeout: Duration) -> Self {
        self.nats_request_timeout = Some(timeout);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_config() {
        let config = NatsConfig::new("nats://localhost:4222", "my-token");
        assert_eq!(config.nats_url, "nats://localhost:4222");
        assert_eq!(config.nats_token, "my-token");
        assert_eq!(config.name(), "nvisy-nats");
        assert_eq!(config.nats_connect_timeout, None);
        assert_eq!(config.nats_request_timeout, None);
        assert_eq!(config.max_reconnects_option(), Some(10));
    }

    #[test]
    fn test_with_request_timeout() {
        let config = NatsConfig::new("nats://localhost:4222", "token")
            .with_request_timeout(Duration::from_secs(15));
        assert_eq!(config.nats_request_timeout, Some(Duration::from_secs(15)));
    }

    #[test]
    fn test_unlimited_reconnects() {
        let mut config = NatsConfig::new("nats://localhost:4222", "token");
        config.nats_max_reconnects = Some(0);
        assert_eq!(config.max_reconnects_option(), None); // Unlimited
    }

    #[test]
    fn test_default_values() {
        let config = NatsConfig::new("nats://localhost:4222", "token");
        assert_eq!(config.name(), DEFAULT_NAME);
        assert_eq!(config.reconnect_delay(), DEFAULT_RECONNECT_DELAY);
        assert_eq!(config.ping_interval(), DEFAULT_PING_INTERVAL);
        assert_eq!(config.max_reconnects_option(), Some(DEFAULT_MAX_RECONNECTS));
    }
}
