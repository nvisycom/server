//! NATS error to HTTP error conversion implementation.
//!
//! This module provides conversion from NATS client errors to appropriate HTTP errors
//! with proper status codes and user-friendly messages.

use nvisy_nats::kv::InvalidKvKey;

use super::http_error::{Error as HttpError, ErrorKind};

/// A key the server generated itself (a CSRF state, a proof token, a lock name)
/// failed KV key validation: a server fault, not a bad request. Untrusted keys
/// are parsed at the call site and mapped to a client error there.
impl From<InvalidKvKey> for HttpError {
    fn from(_: InvalidKvKey) -> Self {
        ErrorKind::InternalServerError.with_message("Generated key is not a valid KV key")
    }
}

impl From<nvisy_nats::Error> for HttpError {
    fn from(error: nvisy_nats::Error) -> Self {
        let kind = match &error {
            // The messaging backend is momentarily unreachable or slow:
            // transient, so a retryable 503.
            nvisy_nats::Error::Connection(_) | nvisy_nats::Error::Timeout { .. } => {
                ErrorKind::ServiceUnavailable
            }
            // A payload we framed ourselves, or a server-side messaging
            // operation: never something the caller's request can fix.
            nvisy_nats::Error::Serialization(_) | nvisy_nats::Error::Operation { .. } => {
                ErrorKind::InternalServerError
            }
        };
        let message = match kind {
            ErrorKind::ServiceUnavailable => "Messaging is temporarily unavailable",
            _ => "Messaging operation failed",
        };
        kind.with_message(message).with_context(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn transient_failures_are_retryable() {
        let timeout = nvisy_nats::Error::Timeout {
            operation: "ping",
            timeout: Duration::from_secs(10),
        };
        assert_eq!(
            HttpError::from(timeout).kind(),
            ErrorKind::ServiceUnavailable
        );
    }

    #[test]
    fn payload_failures_are_server_faults() {
        let json = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let error = HttpError::from(nvisy_nats::Error::Serialization(json));
        assert_eq!(error.kind(), ErrorKind::InternalServerError);
    }
}
