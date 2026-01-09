//! Reqwest client configuration.

use std::time::Duration;

#[cfg(feature = "config")]
use clap::Args;
use serde::{Deserialize, Serialize};

/// Default timeout for HTTP requests: 30 seconds.
pub const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Configuration for the reqwest HTTP client.
///
/// This configuration is used for webhook delivery and other HTTP operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "config", derive(Args))]
pub struct ReqwestConfig {
    /// HTTP request timeout in seconds
    #[cfg_attr(
        feature = "config",
        arg(long = "http-timeout", env = "HTTP_TIMEOUT", default_value = "30")
    )]
    #[serde(default = "default_timeout_secs")]
    pub http_timeout: u64,

    /// User-Agent header to send with requests
    #[cfg_attr(
        feature = "config",
        arg(long = "http-user-agent", env = "HTTP_USER_AGENT")
    )]
    #[serde(default)]
    pub user_agent: Option<String>,
}

fn default_timeout_secs() -> u64 {
    DEFAULT_TIMEOUT_SECS
}

impl Default for ReqwestConfig {
    fn default() -> Self {
        Self {
            http_timeout: default_timeout_secs(),
            user_agent: None,
        }
    }
}

impl ReqwestConfig {
    /// Create a new configuration with the specified timeout.
    pub fn new(timeout_secs: u64) -> Self {
        Self {
            http_timeout: timeout_secs,
            user_agent: None,
        }
    }

    /// Returns the timeout as a Duration.
    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.http_timeout)
    }

    /// Returns the effective timeout, using default if zero.
    pub fn effective_timeout(&self) -> Duration {
        if self.http_timeout == 0 {
            Duration::from_secs(DEFAULT_TIMEOUT_SECS)
        } else {
            Duration::from_secs(self.http_timeout)
        }
    }

    /// Returns the effective user agent, using default if not set.
    pub fn effective_user_agent(&self) -> String {
        self.user_agent
            .clone()
            .unwrap_or_else(Self::default_user_agent)
    }

    /// Returns the default user agent string.
    fn default_user_agent() -> String {
        format!("nvisy/{}", env!("CARGO_PKG_VERSION"))
    }

    /// Set the timeout in seconds.
    #[must_use]
    pub fn with_timeout(mut self, timeout_secs: u64) -> Self {
        self.http_timeout = timeout_secs;
        self
    }

    /// Set the user agent.
    #[must_use]
    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ReqwestConfig::default();
        assert_eq!(config.http_timeout, 30);
        assert!(config.user_agent.is_none());
        assert_eq!(config.timeout(), Duration::from_secs(30));
    }

    #[test]
    fn test_new_config() {
        let config = ReqwestConfig::new(60);
        assert_eq!(config.http_timeout, 60);
        assert_eq!(config.timeout(), Duration::from_secs(60));
    }

    #[test]
    fn test_builder_pattern() {
        let config = ReqwestConfig::default()
            .with_timeout(120)
            .with_user_agent("custom-agent/1.0");

        assert_eq!(config.http_timeout, 120);
        assert_eq!(config.user_agent, Some("custom-agent/1.0".to_string()));
    }

    #[test]
    fn test_effective_timeout_uses_default_when_zero() {
        let config = ReqwestConfig::new(0);
        assert_eq!(
            config.effective_timeout(),
            Duration::from_secs(DEFAULT_TIMEOUT_SECS)
        );
    }

    #[test]
    fn test_effective_user_agent_uses_default_when_none() {
        let config = ReqwestConfig::default();
        assert!(config.effective_user_agent().contains("nvisy"));
    }
}
