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
//!
//! ## Examples
//!
//! Creating a client with the builder pattern:
//!
//! ```no_run
//! # use nvisy_openrouter::{LlmConfig, Result};
//! # fn example() -> Result<()> {
//! let client = LlmConfig::builder()
//!     .with_api_key("your-api-key")
//!     .with_default_model("openai/gpt-4")
//!     .build_client()?;
//! # Ok(())
//! # }
//! ```
//!
//! Creating a client with just an API key:
//!
//! ```no_run
//! # use nvisy_openrouter::{LlmClient, Result};
//! # fn example() -> Result<()> {
//! let client = LlmClient::from_api_key("your-api-key")?;
//! # Ok(())
//! # }
//! ```

use openrouter_rs::error::OpenRouterError;

/// Logging target for OpenRouter client operations.
pub const TRACING_TARGET_CLIENT: &str = "nvisy_openrouter::client";

/// Logging target for configuration operations.
pub const TRACING_TARGET_CONFIG: &str = "nvisy_openrouter::config";

/// Logging target for schema generation and validation.
pub const TRACING_TARGET_SCHEMA: &str = "nvisy_openrouter::schema";

/// Logging target for completion operations.
pub const TRACING_TARGET_COMPLETION: &str = "nvisy_openrouter::completion";

// Core modules
pub mod client;
pub mod completion;
pub mod typed;

pub use client::{LlmClient, LlmConfig};

use crate::client::llm_config::LlmBuilderError;
use crate::completion::{TypedChatRequestBuilderError, TypedChatResponseBuilderError};

/// Result type for OpenRouter operations.
///
/// This is a type alias for `std::result::Result<T, Error>` where `Error` is the
/// crate's error type. Use this in function signatures for consistency.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Comprehensive error types for OpenRouter operations.
///
/// This enum covers all possible failure modes when interacting with the OpenRouter API,
/// providing detailed context and appropriate error handling strategies.
///
/// # Examples
///
/// Handling different error types:
///
/// ```no_run
/// # use nvisy_openrouter::{LlmClient, Error, Result};
/// # async fn example() -> Result<()> {
/// let client = LlmClient::from_api_key("your-api-key")?;
/// let models = client.list_models().await;
///
/// match models {
///     Ok(models) => println!("Found {} models", models.len()),
///     Err(Error::RateLimit { message, retry_after }) => {
///         println!("Rate limited: {}", message);
///         if let Some(duration) = retry_after {
///             println!("Retry after {:?}", duration);
///         }
///     }
///     Err(Error::Api(e)) => println!("API error: {}", e),
///     Err(e) => println!("Other error: {}", e),
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// API-related errors from the OpenRouter service.
    #[error(transparent)]
    Api(#[from] Box<OpenRouterError>),

    /// Rate limiting errors.
    #[error("Rate limit exceeded: {message}")]
    RateLimit {
        /// Details about the rate limit violation
        message: String,
        /// Time until rate limit resets (if known)
        retry_after: Option<std::time::Duration>,
    },

    /// Configuration errors.
    #[error("Configuration error: {0}")]
    Config(#[from] LlmBuilderError),

    /// Typed request error.
    #[error("Typed request error: {0}")]
    TypedRequest(#[from] TypedChatRequestBuilderError),

    /// Typed response error.
    #[error("Typed response error: {0}")]
    TypedResponse(#[from] TypedChatResponseBuilderError),

    /// Invalid response structure errors.
    #[error("Invalid response: {message}")]
    InvalidResponse {
        /// Description of what's invalid
        message: String,
    },

    /// JSON serialization/deserialization errors.
    #[error("JSON serialization error: {0}")]
    JsonSerialization(#[from] serde_json::Error),

    /// TOON serialization/deserialization errors.
    #[error("TOON serialization error: {0}")]
    ToonSerialization(#[from] serde_toon::Error),
}

impl Error {
    /// Creates an invalid response error.
    ///
    /// # Examples
    ///
    /// ```
    /// # use nvisy_openrouter::Error;
    /// let error = Error::invalid_response("Missing required field 'model'");
    /// assert!(matches!(error, Error::InvalidResponse { .. }));
    /// ```
    pub fn invalid_response(message: impl Into<String>) -> Self {
        Self::InvalidResponse {
            message: message.into(),
        }
    }

    /// Creates a rate limit error.
    ///
    /// # Examples
    ///
    /// ```
    /// # use nvisy_openrouter::Error;
    /// # use std::time::Duration;
    /// let error = Error::rate_limit("Too many requests", Some(Duration::from_secs(60)));
    /// assert!(matches!(error, Error::RateLimit { .. }));
    /// ```
    pub fn rate_limit(
        message: impl Into<String>,
        retry_after: Option<std::time::Duration>,
    ) -> Self {
        Self::RateLimit {
            message: message.into(),
            retry_after,
        }
    }

    /// Returns the HTTP status code if available.
    ///
    /// # Examples
    ///
    /// ```
    /// # use nvisy_openrouter::Error;
    /// let error = Error::invalid_response("Invalid configuration");
    /// assert_eq!(error.status_code(), None);
    /// ```
    pub fn status_code(&self) -> Option<u16> {
        match self {
            Self::Api(x) => match x.as_ref() {
                OpenRouterError::ApiError { code, .. } => Some(*code as u16),
                _ => None,
            },
            _ => None,
        }
    }

    /// Returns the retry delay if this error provides one.
    ///
    /// # Examples
    ///
    /// ```
    /// # use nvisy_openrouter::Error;
    /// # use std::time::Duration;
    /// let error = Error::rate_limit("Too many requests", Some(Duration::from_secs(30)));
    /// assert_eq!(error.retry_after(), Some(Duration::from_secs(30)));
    /// ```
    pub fn retry_after(&self) -> Option<std::time::Duration> {
        match self {
            Self::RateLimit { retry_after, .. } => *retry_after,
            _ => None,
        }
    }
}

impl From<OpenRouterError> for Error {
    fn from(value: OpenRouterError) -> Self {
        Self::Api(Box::new(value))
    }
}
