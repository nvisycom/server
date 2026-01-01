//! Configuration for webhook client.

use std::time::Duration;

use crate::error::{Error, Result};

/// Configuration for the webhook client.
#[derive(Debug, Clone)]
pub struct WebhookClientConfig {
    /// Default timeout for webhook requests.
    pub timeout: Duration,
    /// User-Agent header to send with requests.
    pub user_agent: String,
}

impl Default for WebhookClientConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            user_agent: format!("nvisy-webhook/{}", env!("CARGO_PKG_VERSION")),
        }
    }
}

impl WebhookClientConfig {
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

    /// Validates the configuration.
    pub fn validate(&self) -> Result<()> {
        if self.timeout.is_zero() {
            return Err(Error::Config("timeout cannot be zero".into()));
        }
        if self.user_agent.is_empty() {
            return Err(Error::Config("user_agent cannot be empty".into()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = WebhookClientConfig::default();
        assert_eq!(config.timeout, Duration::from_secs(30));
        assert!(config.user_agent.contains("nvisy-webhook"));
    }

    #[test]
    fn test_config_validation() {
        let config = WebhookClientConfig::default();
        assert!(config.validate().is_ok());

        let bad_config = WebhookClientConfig {
            timeout: Duration::ZERO,
            ..Default::default()
        };
        assert!(bad_config.validate().is_err());
    }
}
