//! Configuration for reqwest client.

use std::time::Duration;

/// Default timeout for HTTP requests: 30 seconds.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Configuration for the reqwest HTTP client.
#[derive(Debug, Clone)]
pub struct ReqwestClientConfig {
    /// Default timeout for HTTP requests.
    pub timeout: Duration,
    /// User-Agent header to send with requests.
    pub user_agent: String,
}

impl Default for ReqwestClientConfig {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
            user_agent: Self::default_user_agent(),
        }
    }
}

impl ReqwestClientConfig {
    /// Returns the default user agent string.
    fn default_user_agent() -> String {
        format!("nvisy/{}", env!("CARGO_PKG_VERSION"))
    }

    /// Creates a new configuration with the specified timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Creates a new configuration with the specified user agent.
    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = user_agent.into();
        self
    }

    /// Returns the effective timeout, using default if zero.
    pub fn effective_timeout(&self) -> Duration {
        if self.timeout.is_zero() {
            DEFAULT_TIMEOUT
        } else {
            self.timeout
        }
    }

    /// Returns the effective user agent, using default if empty.
    pub fn effective_user_agent(&self) -> String {
        if self.user_agent.is_empty() {
            Self::default_user_agent()
        } else {
            self.user_agent.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = ReqwestClientConfig::default();
        assert_eq!(config.timeout, Duration::from_secs(30));
        assert!(config.user_agent.contains("nvisy"));
    }

    #[test]
    fn test_effective_timeout_uses_default_when_zero() {
        let config = ReqwestClientConfig {
            timeout: Duration::ZERO,
            ..Default::default()
        };
        assert_eq!(config.effective_timeout(), DEFAULT_TIMEOUT);
    }

    #[test]
    fn test_effective_user_agent_uses_default_when_empty() {
        let config = ReqwestClientConfig {
            user_agent: String::new(),
            ..Default::default()
        };
        assert!(config.effective_user_agent().contains("nvisy"));
    }
}
