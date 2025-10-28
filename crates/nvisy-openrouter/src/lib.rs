//! # nvisy-openrouter
//!
//! A high-level, production-ready client for OpenRouter's API with comprehensive
//! error handling, rate limiting, observability, and data redaction capabilities.
//!
//! This crate provides both client functionality and specialized data redaction
//! services, making it easy to integrate OpenRouter's AI models for privacy-focused
//! data processing tasks.
//!
//! ## Features
//!
//! - **Client**: Full-featured OpenRouter API client with rate limiting
//! - **Error Handling**: Comprehensive error types with recovery strategies
//! - **Data Redaction**: Specialized service for identifying sensitive data to redact
//! - **JSON Processing**: Structured input/output format for redaction tasks
//! - **Observability**: Structured logging and metrics integration

// Tracing targets for observability
/// Logging target for OpenRouter client operations.
pub const OPENROUTER_TARGET: &str = "nvisy_openrouter::client";

/// Logging target for redaction service operations.
pub const REDACTION_TARGET: &str = "nvisy_openrouter::redaction";

/// Logging target for schema generation and validation.
pub const SCHEMA_TARGET: &str = "nvisy_openrouter::schema";

// Core modules
pub mod client;
pub mod completion;

// Error handling module
pub mod error {
    //! Comprehensive error handling for OpenRouter API operations.
    //!
    //! This module provides structured error types that cover all possible failure modes
    //! when interacting with the OpenRouter API, including network issues, API errors,
    //! rate limiting, and configuration problems.

    use openrouter_rs::error::OpenRouterError;

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

        /// Builder errors.
        #[error("Builder error: {message}")]
        Builder {
            /// Description of the builder problem
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

        /// Creates a new builder error.
        pub fn builder(message: impl Into<String>) -> Self {
            Self::Builder {
                message: message.into(),
            }
        }
    }

    impl From<OpenRouterError> for Error {
        fn from(error: OpenRouterError) -> Self {
            // Log the error
            tracing::error!(
                target: OPENROUTER_TARGET,
                error = %error,
                "OpenRouter API error occurred"
            );

            match error {
                OpenRouterError::ApiError { code, message } => {
                    let status_code = code as u16;

                    // Match common status codes
                    if status_code == 401 {
                        Error::Auth {
                            message: "Invalid or expired API key".to_string(),
                        }
                    } else if status_code == 429 {
                        Error::RateLimit {
                            message: "API rate limit exceeded".to_string(),
                            retry_after: None,
                        }
                    } else if status_code == 400 {
                        Error::Api {
                            message: "Invalid request parameters".to_string(),
                            status_code: Some(status_code),
                            error_code: None,
                        }
                    } else if status_code == 402 {
                        Error::Api {
                            message: "Insufficient credits or quota exceeded".to_string(),
                            status_code: Some(status_code),
                            error_code: None,
                        }
                    } else if status_code == 404 {
                        Error::Api {
                            message: "Model not found or not available".to_string(),
                            status_code: Some(status_code),
                            error_code: None,
                        }
                    } else {
                        Error::Api {
                            message,
                            status_code: Some(status_code),
                            error_code: None,
                        }
                    }
                }
                OpenRouterError::HttpRequest(err) => {
                    let err_string = err.to_string();
                    let recoverable = !err_string.contains("dns")
                        && !err_string.contains("certificate")
                        && !err_string.contains("tls");

                    if err_string.contains("timeout") {
                        Error::Timeout {
                            timeout_seconds: 30,
                        }
                    } else {
                        Error::Network {
                            message: err_string,
                            recoverable,
                        }
                    }
                }
                OpenRouterError::KeyNotConfigured => Error::Auth {
                    message: "API key not configured".to_string(),
                },
                OpenRouterError::ConfigError(_) | OpenRouterError::ConfigNotFound(_) => {
                    Error::Config {
                        message: error.to_string(),
                    }
                }
                OpenRouterError::ModerationError { message, .. } => Error::Api {
                    message: format!("Content moderation error: {}", message),
                    status_code: None,
                    error_code: Some("moderation".to_string()),
                },
                OpenRouterError::ProviderError {
                    message,
                    provider_name,
                    ..
                } => Error::Api {
                    message: format!("Provider error ({}): {}", provider_name, message),
                    status_code: None,
                    error_code: Some("provider".to_string()),
                },
                _ => Error::Api {
                    message: error.to_string(),
                    status_code: None,
                    error_code: None,
                },
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
    }
}

// Re-export commonly used types for convenience
pub use client::{LlmClient, LlmConfig};
// Re-export completion types
pub use completion::{
    Entity, RedactedData, RedactionCategory, RedactionItem, RedactionRequest, RedactionResponse,
    RedactionService, TypedChatCompletion, TypedChatRequest, TypedChatResponse,
};
pub use error::{Error, Result};
// Re-export OpenRouter API types that users commonly need
pub use openrouter_rs::{
    Model, OpenRouterClient,
    api::chat::{ChatCompletionRequest, Message},
    types::{Role, completion::CompletionsResponse},
};
// Re-export schemars for users who want to define custom schemas
pub use schemars::JsonSchema;

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_public_api_availability() {
        // Test that all main public types are accessible
        let _config = LlmConfig::default();

        // Test redaction types
        let _item = RedactionItem::new("test").with_entity("test_entity");

        let _request = RedactionRequest::new(vec![], "test prompt");

        let _response = RedactionResponse::new();

        let _entity = Entity::new("John Doe").with_category(RedactionCategory::FullNames);
    }

    #[test]
    fn test_error_types() {
        // Test that error types are properly exposed
        let error = Error::config("Test error");
        assert!(error.is_config_error());

        let error = Error::rate_limit("Rate limit");
        assert!(error.is_rate_limit_error());

        let error = Error::auth("Auth error");
        assert!(error.is_auth_error());
    }

    #[test]
    fn test_redaction_integration() {
        // Test that redaction utilities work together
        let item = RedactionItem::new("123 Main St").with_entity("John Doe");

        let request = RedactionRequest::new(vec![item], "Redact all addresses");

        assert_eq!(request.data.len(), 1);
        assert_eq!(request.data[0].text, "123 Main St");
        assert_eq!(request.data[0].entity.as_deref(), Some("John Doe"));
    }

    #[test]
    fn test_redaction_prompt_formatting() {
        use crate::completion::redaction_prompts::{create_system_prompt, create_user_prompt};

        let items = vec![RedactionItem::new("test data").with_entity("test entity")];
        let request = RedactionRequest::new(items, "test redaction");

        let system_prompt = create_system_prompt();
        let user_prompt = create_user_prompt(&request);

        assert!(!system_prompt.is_empty());
        assert!(user_prompt.contains("test data"));
        assert!(user_prompt.contains("test redaction"));
    }
}
