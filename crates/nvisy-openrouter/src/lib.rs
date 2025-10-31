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

use openrouter_rs::error::OpenRouterError;

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

pub use client::{LlmClient, LlmConfig};

/// Result type for OpenRouter operations.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Comprehensive error types for OpenRouter operations.
///
/// This enum covers all possible failure modes when interacting with the OpenRouter API,
/// providing detailed context and appropriate error handling strategies.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// API-related errors from the OpenRouter service.
    #[error(transparent)]
    Api(#[from] OpenRouterError),

    /// Rate limiting errors.
    #[error("Rate limit exceeded: {message}")]
    RateLimit {
        /// Details about the rate limit violation
        message: String,
        /// Time until rate limit resets (if known)
        retry_after: Option<std::time::Duration>,
    },

    /// Configuration errors.
    #[error("Configuration error: {message}")]
    Config {
        /// Description of the configuration problem
        message: String,
    },

    /// Invalid response structure errors.
    #[error("Invalid response: {message}")]
    InvalidResponse {
        /// Description of what's invalid
        message: String,
    },

    /// Serialization/deserialization errors.
    #[error(transparent)]
    Serialization(#[from] serde_json::Error),
}

impl Error {
    /// Create a configuration error.
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config {
            message: message.into(),
        }
    }

    /// Create an invalid response error.
    pub fn invalid_response(message: impl Into<String>) -> Self {
        Self::InvalidResponse {
            message: message.into(),
        }
    }

    /// Create a rate limit error.
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
    pub fn status_code(&self) -> Option<u16> {
        match self {
            Self::Api(OpenRouterError::ApiError { code, .. }) => Some(*code as u16),
            _ => None,
        }
    }

    /// Returns the retry delay if this error provides one.
    pub fn retry_after(&self) -> Option<std::time::Duration> {
        match self {
            Self::RateLimit { retry_after, .. } => *retry_after,
            _ => None,
        }
    }
}
