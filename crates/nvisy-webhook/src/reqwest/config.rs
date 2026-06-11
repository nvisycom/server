//! Reqwest client configuration.

use std::time::Duration;

/// Default timeout for HTTP requests.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Default maximum number of retry attempts.
pub const DEFAULT_MAX_RETRIES: u32 = 3;

/// Default minimum retry interval.
pub const DEFAULT_MIN_RETRY_INTERVAL: Duration = Duration::from_millis(500);

/// Default maximum retry interval.
pub const DEFAULT_MAX_RETRY_INTERVAL: Duration = Duration::from_millis(30_000);

/// Configuration for the reqwest HTTP client.
///
/// This configuration is used for webhook delivery and other HTTP operations.
#[derive(Debug, Clone)]
pub struct ReqwestConfig {
    /// HTTP request timeout (falls back to the default when unset).
    pub http_timeout: Option<Duration>,

    /// User-Agent header to send with requests.
    pub user_agent: Option<String>,

    /// Maximum number of retry attempts for transient failures.
    pub max_retries: u32,

    /// Minimum retry interval.
    pub min_retry_interval: Duration,

    /// Maximum retry interval.
    pub max_retry_interval: Duration,
}

impl Default for ReqwestConfig {
    fn default() -> Self {
        Self {
            http_timeout: None,
            user_agent: None,
            max_retries: DEFAULT_MAX_RETRIES,
            min_retry_interval: DEFAULT_MIN_RETRY_INTERVAL,
            max_retry_interval: DEFAULT_MAX_RETRY_INTERVAL,
        }
    }
}

impl ReqwestConfig {
    /// Create a new configuration with the specified timeout.
    pub fn new(timeout: Duration) -> Self {
        Self {
            http_timeout: Some(timeout),
            ..Default::default()
        }
    }

    /// Returns the effective timeout, using the default when unset.
    pub fn effective_timeout(&self) -> Duration {
        self.http_timeout.unwrap_or(DEFAULT_TIMEOUT)
    }

    /// Returns the effective user agent, using default if not set.
    pub(crate) fn effective_user_agent(&self) -> String {
        self.user_agent
            .clone()
            .unwrap_or_else(Self::default_user_agent)
    }

    /// Returns the default user agent string.
    fn default_user_agent() -> String {
        format!("nvisy/{}", env!("CARGO_PKG_VERSION"))
    }

    /// Set the request timeout.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.http_timeout = Some(timeout);
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

    /// Set the retry interval bounds.
    #[must_use]
    pub fn with_retry_interval(mut self, min: Duration, max: Duration) -> Self {
        self.min_retry_interval = min;
        self.max_retry_interval = max;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ReqwestConfig::default();
        assert_eq!(config.http_timeout, None);
        assert!(config.user_agent.is_none());
        assert_eq!(config.effective_timeout(), DEFAULT_TIMEOUT);
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.min_retry_interval, Duration::from_millis(500));
        assert_eq!(config.max_retry_interval, Duration::from_millis(30_000));
    }

    #[test]
    fn test_new_config() {
        let config = ReqwestConfig::new(Duration::from_secs(60));
        assert_eq!(config.http_timeout, Some(Duration::from_secs(60)));
        assert_eq!(config.effective_timeout(), Duration::from_secs(60));
        assert_eq!(config.max_retries, DEFAULT_MAX_RETRIES);
    }

    #[test]
    fn test_builder_pattern() {
        let config = ReqwestConfig::default()
            .with_timeout(Duration::from_secs(120))
            .with_user_agent("custom-agent/1.0")
            .with_max_retries(5)
            .with_retry_interval(Duration::from_secs(1), Duration::from_secs(60));

        assert_eq!(config.http_timeout, Some(Duration::from_secs(120)));
        assert_eq!(config.user_agent, Some("custom-agent/1.0".to_string()));
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.min_retry_interval, Duration::from_secs(1));
        assert_eq!(config.max_retry_interval, Duration::from_secs(60));
    }

    #[test]
    fn test_effective_timeout_uses_default_when_unset() {
        let config = ReqwestConfig::default();
        assert_eq!(config.effective_timeout(), DEFAULT_TIMEOUT);
    }

    #[test]
    fn test_effective_user_agent_uses_default_when_none() {
        let config = ReqwestConfig::default();
        assert!(config.effective_user_agent().contains("nvisy"));
    }
}
