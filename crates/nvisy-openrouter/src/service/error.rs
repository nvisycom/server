//! Comprehensive error handling for OpenRouter API operations.
//!
//! This module provides structured error types that cover all possible failure modes
//! when interacting with the OpenRouter API, including network issues, API errors,
//! rate limiting, and configuration problems.

use nvisy_error::UpdateSeverity;
use openrouter_api::error::Error as ApiError;

use crate::OPENROUTER_TARGET;

/// Result type for OpenRouter operations.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Comprehensive error types for OpenRouter operations.
///
/// This enum covers all possible failure modes when interacting with the OpenRouter API,
/// providing detailed context and appropriate error handling strategies.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// API-related errors from the OpenRouter service.
    #[error("OpenRouter API error: {message}")]
    Api {
        /// The error message from the API
        message: String,
        /// HTTP status code if available
        status_code: Option<u16>,
        /// Error code from OpenRouter if available
        error_code: Option<String>,
    },

    /// Network connectivity issues.
    #[error("Network error: {message}")]
    Network {
        /// Description of the network error
        message: String,
        /// Whether this error might be recoverable
        recoverable: bool,
    },

    /// Rate limiting errors.
    #[error("Rate limit exceeded: {message}")]
    RateLimit {
        /// Details about the rate limit violation
        message: String,
        /// Time until rate limit resets (if known)
        retry_after: Option<std::time::Duration>,
    },

    /// Authentication and authorization errors.
    #[error("Authentication error: {message}")]
    Auth {
        /// Details about the authentication failure
        message: String,
    },

    /// Request timeout errors.
    #[error("Request timeout: operation took longer than {timeout_seconds}s")]
    Timeout {
        /// The timeout duration in seconds
        timeout_seconds: u64,
    },

    /// Configuration errors.
    #[error("Configuration error: {message}")]
    Config {
        /// Description of the configuration problem
        message: String,
    },
}

impl Error {
    /// Returns true if this error might be recoverable with a retry.
    pub fn is_recoverable(&self) -> bool {
        match self {
            Error::Network { recoverable, .. } => *recoverable,
            Error::Timeout { .. } => true,
            Error::RateLimit { .. } => true,
            Error::Api { status_code, .. } => {
                // 5xx errors are generally recoverable
                status_code
                    .map(|code| code >= 500 && code < 600)
                    .unwrap_or(false)
            }
            _ => false,
        }
    }

    /// Returns true if this error is related to rate limiting.
    pub fn is_rate_limit_error(&self) -> bool {
        matches!(self, Error::RateLimit { .. })
    }

    /// Returns true if this error is related to authentication.
    pub fn is_auth_error(&self) -> bool {
        matches!(self, Error::Auth { .. })
    }

    /// Returns true if this error is related to configuration.
    pub fn is_config_error(&self) -> bool {
        matches!(self, Error::Config { .. })
    }

    /// Returns the HTTP status code if available.
    pub fn status_code(&self) -> Option<u16> {
        match self {
            Error::Api { status_code, .. } => *status_code,
            _ => None,
        }
    }

    /// Returns the retry delay if this error provides one.
    pub fn retry_after(&self) -> Option<std::time::Duration> {
        match self {
            Error::RateLimit { retry_after, .. } => *retry_after,
            _ => None,
        }
    }

    /// Returns the error severity level for monitoring.
    pub fn severity(&self) -> UpdateSeverity {
        match self {
            Error::Config { .. } => UpdateSeverity::Critical,
            Error::Auth { .. } => UpdateSeverity::Critical,
            Error::RateLimit { .. } => UpdateSeverity::Warning,
            Error::Timeout { .. } => UpdateSeverity::Warning,
            Error::Network {
                recoverable: true, ..
            } => UpdateSeverity::Warning,
            Error::Network {
                recoverable: false, ..
            } => UpdateSeverity::Critical,
            Error::Api {
                status_code: Some(code),
                ..
            } if *code >= 500 => UpdateSeverity::Warning,
            Error::Api {
                status_code: Some(code),
                ..
            } if *code >= 400 => UpdateSeverity::Critical,
            Error::Api { .. } => UpdateSeverity::Warning,
        }
    }
}

/// Converts the underlying OpenRouter API error to our structured error type.
pub fn convert_api_error(error: ApiError) -> Error {
    let error_string = error.to_string();

    // Log the error
    tracing::error!(
        target: OPENROUTER_TARGET,
        error = %error_string,
        "OpenRouter API error occurred"
    );

    // Try to extract useful information from the error
    if error_string.contains("401") || error_string.to_lowercase().contains("unauthorized") {
        Error::Auth {
            message: "Invalid or expired API key".to_string(),
        }
    } else if error_string.contains("429") || error_string.to_lowercase().contains("rate limit") {
        Error::RateLimit {
            message: "API rate limit exceeded".to_string(),
            retry_after: None, // Could be extracted from headers in a more sophisticated implementation
        }
    } else if error_string.contains("400") || error_string.to_lowercase().contains("bad request") {
        Error::Api {
            message: "Invalid request parameters".to_string(),
            status_code: Some(400),
            error_code: None,
        }
    } else if error_string.contains("402") || error_string.to_lowercase().contains("payment") {
        Error::Api {
            message: "Insufficient credits or quota exceeded".to_string(),
            status_code: Some(402),
            error_code: None,
        }
    } else if error_string.contains("404") {
        Error::Api {
            message: "Model not found or not available".to_string(),
            status_code: Some(404),
            error_code: None,
        }
    } else if error_string.contains("timeout") {
        Error::Timeout {
            timeout_seconds: 30, // Default assumption
        }
    } else if error_string.to_lowercase().contains("network")
        || error_string.to_lowercase().contains("connection")
    {
        Error::Network {
            message: error_string.clone(),
            recoverable: true,
        }
    } else if let Some(status_code) = extract_status_code(&error_string) {
        Error::Api {
            message: error_string,
            status_code: Some(status_code),
            error_code: None,
        }
    } else {
        Error::Api {
            message: error_string,
            status_code: None,
            error_code: None,
        }
    }
}

/// Helper function to extract HTTP status codes from error messages.
fn extract_status_code(error_msg: &str) -> Option<u16> {
    // Look for patterns like "500", "404", etc. in the error message
    let words: Vec<&str> = error_msg.split_whitespace().collect();

    for word in words {
        if let Ok(code) = word.parse::<u16>() {
            if code >= 100 && code < 600 {
                return Some(code);
            }
        }
    }

    None
}

/// Helper functions for creating specific error types.
impl Error {
    /// Creates a new API error.
    pub fn api(message: impl Into<String>) -> Self {
        Self::Api {
            message: message.into(),
            status_code: None,
            error_code: None,
        }
    }

    /// Creates a new API error with status code.
    pub fn api_with_status(message: impl Into<String>, status_code: u16) -> Self {
        Self::Api {
            message: message.into(),
            status_code: Some(status_code),
            error_code: None,
        }
    }

    /// Creates a new network error.
    pub fn network(message: impl Into<String>, recoverable: bool) -> Self {
        Self::Network {
            message: message.into(),
            recoverable,
        }
    }

    /// Creates a new rate limit error.
    pub fn rate_limit(message: impl Into<String>) -> Self {
        Self::RateLimit {
            message: message.into(),
            retry_after: None,
        }
    }

    /// Creates a new rate limit error with retry delay.
    pub fn rate_limit_with_retry(
        message: impl Into<String>,
        retry_after: std::time::Duration,
    ) -> Self {
        Self::RateLimit {
            message: message.into(),
            retry_after: Some(retry_after),
        }
    }

    /// Creates a new authentication error.
    pub fn auth(message: impl Into<String>) -> Self {
        Self::Auth {
            message: message.into(),
        }
    }

    /// Creates a new timeout error.
    pub fn timeout(seconds: u64) -> Self {
        Self::Timeout {
            timeout_seconds: seconds,
        }
    }

    /// Creates a new configuration error.
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config {
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_recoverability() {
        let recoverable_network = Error::network("Connection failed", true);
        assert!(recoverable_network.is_recoverable());

        let non_recoverable_network = Error::network("DNS resolution failed", false);
        assert!(!non_recoverable_network.is_recoverable());

        let timeout_error = Error::timeout(30);
        assert!(timeout_error.is_recoverable());

        let auth_error = Error::auth("Invalid API key");
        assert!(!auth_error.is_recoverable());
    }

    #[test]
    fn test_error_classification() {
        let rate_limit_error = Error::rate_limit("Too many requests");
        assert!(rate_limit_error.is_rate_limit_error());
        assert!(!rate_limit_error.is_auth_error());

        let auth_error = Error::auth("Invalid token");
        assert!(auth_error.is_auth_error());
        assert!(!auth_error.is_rate_limit_error());
    }

    #[test]
    fn test_status_code_extraction() {
        assert_eq!(extract_status_code("HTTP 404 Not Found"), Some(404));
        assert_eq!(extract_status_code("Error 500 occurred"), Some(500));
        assert_eq!(extract_status_code("Something went wrong"), None);
        assert_eq!(extract_status_code("Invalid code 999"), None); // Outside valid range
    }

    #[test]
    fn test_severity_levels() {
        let auth_error = Error::auth("Invalid key");
        assert_eq!(auth_error.severity(), UpdateSeverity::Critical);

        let rate_limit_error = Error::rate_limit("Too many requests");
        assert_eq!(rate_limit_error.severity(), UpdateSeverity::Warning);

        let api_error = Error::api_with_status("Server error", 500);
        assert_eq!(api_error.severity(), UpdateSeverity::Warning);

        let api_error_4xx = Error::api_with_status("Bad request", 400);
        assert_eq!(api_error_4xx.severity(), UpdateSeverity::Critical);
    }
}
