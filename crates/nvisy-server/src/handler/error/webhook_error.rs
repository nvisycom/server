//! Webhook error to HTTP error conversion.
//!
//! Converts [`nvisy_webhook::Error`] into the handler HTTP error with
//! appropriate status codes, user-facing messages, and observability logging.

use super::http_error::{Error as HttpError, ErrorKind};

/// Tracing target for webhook error conversions.
const TRACING_TARGET: &str = "nvisy_server::handler::webhook";

impl From<nvisy_webhook::Error> for HttpError<'static> {
    fn from(error: nvisy_webhook::Error) -> Self {
        use nvisy_webhook::ErrorKind as WebhookErrorKind;

        // Log the error with appropriate level based on error kind
        match error.kind {
            WebhookErrorKind::DeliveryFailed | WebhookErrorKind::Timeout => {
                tracing::warn!(
                    target: TRACING_TARGET,
                    error = %error,
                    error_kind = ?error.kind,
                    "Webhook delivery failed"
                );
            }
            WebhookErrorKind::Configuration => {
                tracing::error!(
                    target: TRACING_TARGET,
                    error = %error,
                    "Invalid webhook configuration"
                );
            }
            WebhookErrorKind::SignatureError | WebhookErrorKind::Serialization => {
                tracing::error!(
                    target: TRACING_TARGET,
                    error = %error,
                    error_kind = ?error.kind,
                    "Webhook payload processing failed"
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
            WebhookErrorKind::InvalidEndpoint => ErrorKind::BadRequest
                .with_message("Invalid webhook endpoint")
                .with_context(message.to_string()),

            WebhookErrorKind::Configuration => ErrorKind::BadRequest
                .with_message("Invalid webhook configuration")
                .with_context(message.to_string()),

            WebhookErrorKind::Timeout => ErrorKind::InternalServerError
                .with_message("Webhook delivery timed out")
                .with_context("The endpoint did not respond in time"),

            WebhookErrorKind::DeliveryFailed | WebhookErrorKind::NonRetryableStatus => {
                ErrorKind::InternalServerError
                    .with_message("Webhook delivery failed")
                    .with_context(message.to_string())
            }

            WebhookErrorKind::SignatureError | WebhookErrorKind::Serialization => {
                ErrorKind::InternalServerError
                    .with_message("Webhook payload processing failed")
                    .with_context(message.to_string())
            }

            WebhookErrorKind::Unknown => ErrorKind::InternalServerError
                .with_message("Internal error")
                .with_context(message.to_string()),
        }
    }
}
