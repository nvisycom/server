//! Service error to HTTP error conversion implementation.
//!
//! This module provides conversion from nvisy-service errors to appropriate HTTP errors
//! with proper status codes, user-friendly messages, and observability logging.

use super::http_error::{Error as HttpError, ErrorKind};

/// Tracing target for service error conversions.
const TRACING_TARGET: &str = "nvisy_server::handler::service";

impl From<nvisy_service::Error> for HttpError<'static> {
    fn from(error: nvisy_service::Error) -> Self {
        use nvisy_service::ErrorKind as ServiceErrorKind;

        // Log the error with appropriate level based on error kind
        match error.kind {
            ServiceErrorKind::NetworkError | ServiceErrorKind::Timeout => {
                tracing::warn!(
                    target: TRACING_TARGET,
                    error = %error,
                    error_kind = ?error.kind,
                    "Service request failed"
                );
            }
            ServiceErrorKind::Configuration => {
                tracing::error!(
                    target: TRACING_TARGET,
                    error = %error,
                    "Invalid service configuration"
                );
            }
            ServiceErrorKind::Serialization => {
                tracing::error!(
                    target: TRACING_TARGET,
                    error = %error,
                    "Serialization failed"
                );
            }
            ServiceErrorKind::InvalidInput => {
                tracing::warn!(
                    target: TRACING_TARGET,
                    error = %error,
                    "Invalid input"
                );
            }
            ServiceErrorKind::Authentication | ServiceErrorKind::Authorization => {
                tracing::warn!(
                    target: TRACING_TARGET,
                    error = %error,
                    error_kind = ?error.kind,
                    "Authentication/authorization failed"
                );
            }
            _ => {
                tracing::warn!(
                    target: TRACING_TARGET,
                    error = %error,
                    error_kind = ?error.kind,
                    "Service operation failed"
                );
            }
        }

        // Convert to appropriate HTTP error
        let message = error
            .message
            .as_deref()
            .unwrap_or("Service operation failed");

        match error.kind {
            ServiceErrorKind::InvalidInput => ErrorKind::BadRequest
                .with_message("Invalid input")
                .with_context(message.to_string()),

            ServiceErrorKind::Configuration => ErrorKind::BadRequest
                .with_message("Invalid configuration")
                .with_context(message.to_string()),

            ServiceErrorKind::Authentication => ErrorKind::Unauthorized
                .with_message("Authentication failed")
                .with_context(message.to_string()),

            ServiceErrorKind::Authorization => ErrorKind::Forbidden
                .with_message("Access denied")
                .with_context(message.to_string()),

            ServiceErrorKind::NotFound => ErrorKind::NotFound
                .with_message("Resource not found")
                .with_context(message.to_string()),

            ServiceErrorKind::RateLimited => ErrorKind::TooManyRequests
                .with_message("Rate limit exceeded")
                .with_context(message.to_string()),

            ServiceErrorKind::Timeout => ErrorKind::InternalServerError
                .with_message("Request timed out")
                .with_context("The service did not respond in time"),

            ServiceErrorKind::NetworkError => ErrorKind::InternalServerError
                .with_message("Network error")
                .with_context(message.to_string()),

            ServiceErrorKind::Serialization => ErrorKind::InternalServerError
                .with_message("Serialization failed")
                .with_context(message.to_string()),

            ServiceErrorKind::ServiceUnavailable
            | ServiceErrorKind::InternalError
            | ServiceErrorKind::ExternalError
            | ServiceErrorKind::Unknown => ErrorKind::InternalServerError
                .with_message("Internal error")
                .with_context(message.to_string()),
        }
    }
}
