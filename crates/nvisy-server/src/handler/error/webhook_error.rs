//! Webhook error to HTTP error conversion implementation.
//!
//! This module provides conversion from webhook client errors to appropriate HTTP errors
//! with proper status codes, user-friendly messages, and observability logging.

use super::http_error::{Error as HttpError, ErrorKind};

/// Tracing target for webhook error conversions.
const TRACING_TARGET: &str = "nvisy_server::handler::webhook";

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
                    "Webhook delivery failed"
                );
            }
            ServiceErrorKind::Configuration => {
                tracing::error!(
                    target: TRACING_TARGET,
                    error = %error,
                    "Invalid webhook configuration"
                );
            }
            ServiceErrorKind::Serialization => {
                tracing::error!(
                    target: TRACING_TARGET,
                    error = %error,
                    "Webhook payload serialization failed"
                );
            }
            _ => {
                tracing::warn!(
                    target: TRACING_TARGET,
                    error = %error,
                    error_kind = ?error.kind,
                    "Webhook operation failed"
                );
            }
        }

        // Convert to appropriate HTTP error
        let message = error
            .message
            .as_deref()
            .unwrap_or("Webhook operation failed");

        match error.kind {
            ServiceErrorKind::Configuration => ErrorKind::BadRequest
                .with_message("Invalid webhook configuration")
                .with_context(message.to_string()),

            ServiceErrorKind::Serialization => ErrorKind::InternalServerError
                .with_message("Failed to serialize webhook payload")
                .with_context(message.to_string()),

            ServiceErrorKind::Timeout => ErrorKind::InternalServerError
                .with_message("Webhook request timed out")
                .with_context("The webhook endpoint did not respond in time"),

            ServiceErrorKind::NetworkError => ErrorKind::InternalServerError
                .with_message("Webhook delivery failed")
                .with_context(message.to_string()),

            _ => ErrorKind::InternalServerError
                .with_message("Webhook operation failed")
                .with_context(message.to_string()),
        }
    }
}
