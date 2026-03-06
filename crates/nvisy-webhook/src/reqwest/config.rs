//! Reqwest client configuration.

use std::time::Duration;

#[cfg(feature = "config")]
use clap::Args;
use serde::{Deserialize, Serialize};

/// Default timeout for HTTP requests: 30 seconds.
pub const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Default maximum number of retry attempts.
pub const DEFAULT_MAX_RETRIES: u32 = 3;

/// Default minimum retry interval in milliseconds.
pub const DEFAULT_MIN_RETRY_INTERVAL_MS: u64 = 500;

/// Default maximum retry interval in milliseconds.
pub const DEFAULT_MAX_RETRY_INTERVAL_MS: u64 = 30_000;

/// Configuration for the reqwest HTTP client.
///
/// This configuration is used for webhook delivery and other HTTP operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "config", derive(Args))]
pub struct ReqwestConfig {
    /// HTTP request timeout in seconds.
    #[cfg_attr(
        feature = "config",
        arg(long = "http-timeout", env = "HTTP_TIMEOUT", default_value = "30")
    )]
    #[serde(default = "default_timeout_secs")]
    pub http_timeout: u64,

    /// User-Agent header to send with requests.
    #[cfg_attr(
        feature = "config",
        arg(long = "http-user-agent", env = "HTTP_USER_AGENT")
    )]
    #[serde(default)]
    pub user_agent: Option<String>,

    /// Maximum number of retry attempts for transient failures.
    #[cfg_attr(
        feature = "config",
        arg(
            long = "http-max-retries",
            env = "HTTP_MAX_RETRIES",
            default_value = "3"
        )
    )]
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    /// Minimum retry interval in milliseconds.
    #[cfg_attr(
        feature = "config",
        arg(
            long = "http-min-retry-interval",
            env = "HTTP_MIN_RETRY_INTERVAL_MS",
            default_value = "500"
        )
    )]
    #[serde(default = "default_min_retry_interval_ms")]
    pub min_retry_interval_ms: u64,

    /// Maximum retry interval in milliseconds.
    #[cfg_attr(
        feature = "config",
        arg(
            long = "http-max-retry-interval",
            env = "HTTP_MAX_RETRY_INTERVAL_MS",
            default_value = "30000"
        )
    )]
    #[serde(default = "default_max_retry_interval_ms")]
    pub max_retry_interval_ms: u64,
}

fn default_timeout_secs() -> u64 {
    DEFAULT_TIMEOUT_SECS
}

fn default_max_retries() -> u32 {
    DEFAULT_MAX_RETRIES
}

fn default_min_retry_interval_ms() -> u64 {
    DEFAULT_MIN_RETRY_INTERVAL_MS
}

fn default_max_retry_interval_ms() -> u64 {
    DEFAULT_MAX_RETRY_INTERVAL_MS
}

impl Default for ReqwestConfig {
    fn default() -> Self {
        Self {
            http_timeout: default_timeout_secs(),
            user_agent: None,
            max_retries: default_max_retries(),
            min_retry_interval_ms: default_min_retry_interval_ms(),
            max_retry_interval_ms: default_max_retry_interval_ms(),
        }
    }
}

impl ReqwestConfig {
    /// Create a new configuration with the specified timeout.
    pub fn new(timeout_secs: u64) -> Self {
        Self {
            http_timeout: timeout_secs,
            ..Default::default()
        }
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

    /// Returns the minimum retry interval as a Duration.
    pub fn min_retry_interval(&self) -> Duration {
        Duration::from_millis(self.min_retry_interval_ms)
    }

    /// Returns the maximum retry interval as a Duration.
    pub fn max_retry_interval(&self) -> Duration {
        Duration::from_millis(self.max_retry_interval_ms)
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

    /// Set the maximum number of retry attempts.
    #[must_use]
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Set the retry interval bounds in milliseconds.
    #[must_use]
    pub fn with_retry_interval(mut self, min_ms: u64, max_ms: u64) -> Self {
        self.min_retry_interval_ms = min_ms;
        self.max_retry_interval_ms = max_ms;
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
        assert_eq!(config.effective_timeout(), Duration::from_secs(30));
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.min_retry_interval_ms, 500);
        assert_eq!(config.max_retry_interval_ms, 30_000);
    }

    #[test]
    fn test_new_config() {
        let config = ReqwestConfig::new(60);
        assert_eq!(config.http_timeout, 60);
        assert_eq!(config.effective_timeout(), Duration::from_secs(60));
        assert_eq!(config.max_retries, DEFAULT_MAX_RETRIES);
    }

    #[test]
    fn test_builder_pattern() {
        let config = ReqwestConfig::default()
            .with_timeout(120)
            .with_user_agent("custom-agent/1.0")
            .with_max_retries(5)
            .with_retry_interval(1000, 60_000);

        assert_eq!(config.http_timeout, 120);
        assert_eq!(config.user_agent, Some("custom-agent/1.0".to_string()));
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.min_retry_interval_ms, 1000);
        assert_eq!(config.max_retry_interval_ms, 60_000);
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

    #[test]
    fn test_retry_interval_durations() {
        let config = ReqwestConfig::default();
        assert_eq!(config.min_retry_interval(), Duration::from_millis(500));
        assert_eq!(config.max_retry_interval(), Duration::from_millis(30_000));
    }
}
