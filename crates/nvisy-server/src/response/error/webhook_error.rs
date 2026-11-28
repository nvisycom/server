//! Webhook error to HTTP error conversion.
//!
//! Converts [`nvisy_webhook::Error`] into the handler HTTP error with
//! appropriate status codes and user-facing messages.

use super::http_error::{Error as HttpError, ErrorKind};

impl From<nvisy_webhook::Error> for HttpError {
    fn from(error: nvisy_webhook::Error) -> Self {
        use nvisy_webhook::ErrorKind as WebhookErrorKind;

        let message = error
            .message
            .as_deref()
            .unwrap_or("Webhook operation failed");

        match error.kind {
            WebhookErrorKind::InvalidEndpoint => ErrorKind::BadRequest
                .with_message("Invalid webhook endpoint")
                .with_context(message.to_string()),

            WebhookErrorKind::Configuration => ErrorKind::InternalServerError
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
