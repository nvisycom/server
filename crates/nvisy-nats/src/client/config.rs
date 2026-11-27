//! NATS connection configuration.

use std::fmt;
use std::time::Duration;

/// Configuration for NATS connections with sensible defaults.
///
/// The token is masked in the [`Debug`] output.
#[derive(Clone)]
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
            value_parser = nvisy_core::parse_duration,
        )
    )]
    pub nats_connect_timeout: Option<Duration>,

    /// Request timeout (optional).
    #[cfg_attr(
        feature = "cli",
        arg(
            long = "nats-request-timeout",
            env = "NATS_REQUEST_TIMEOUT",
            value_parser = nvisy_core::parse_duration,
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

impl fmt::Debug for NatsConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NatsConfig")
            .field("nats_url", &self.redacted_url())
            .field("nats_token", &"<redacted>")
            .field("nats_client_name", &self.nats_client_name)
            .field("nats_connect_timeout", &self.nats_connect_timeout)
            .field("nats_request_timeout", &self.nats_request_timeout)
            .field("nats_max_reconnects", &self.nats_max_reconnects)
            .finish()
    }
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

    /// The server URL(s) with any embedded credentials (`user:pass@`) masked in
    /// each comma-separated entry, for logs and `Debug`.
    #[must_use]
    pub fn redacted_url(&self) -> String {
        self.nats_url
            .split(',')
            .map(|url| match (url.find("://"), url.rfind('@')) {
                (Some(scheme), Some(at)) if at > scheme => {
                    format!("{}<redacted>{}", &url[..scheme + 3], &url[at..])
                }
                (None, Some(at)) => format!("<redacted>{}", &url[at..]),
                _ => url.to_owned(),
            })
            .collect::<Vec<_>>()
            .join(",")
    }

    /// Returns the client name, using the default if not set.
    #[inline]
    #[must_use]
    pub fn name(&self) -> &str {
        self.nats_client_name.as_deref().unwrap_or(DEFAULT_NAME)
    }

    /// Returns the reconnect delay as a Duration.
    #[inline]
    #[must_use]
    pub fn reconnect_delay(&self) -> Duration {
        DEFAULT_RECONNECT_DELAY
    }

    /// Returns the ping interval as a Duration.
    #[inline]
    #[must_use]
    pub fn ping_interval(&self) -> Duration {
        DEFAULT_PING_INTERVAL
    }

    /// Returns the max reconnects as Option (0 means unlimited).
    #[inline]
    #[must_use]
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
    fn redacted_url_masks_credentials_in_every_entry() {
        let config = NatsConfig::new(
            "nats://user:pass@a:4222,nats://b:4222,tls://tok@c:4222",
            "token",
        );
        assert_eq!(
            config.redacted_url(),
            "nats://<redacted>@a:4222,nats://b:4222,tls://<redacted>@c:4222"
        );
        assert!(!format!("{config:?}").contains("pass"));
    }

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
