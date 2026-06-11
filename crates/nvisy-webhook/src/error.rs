//! Domain-specific error handling for webhook delivery.

use hipstr::HipStr;
use strum::{AsRefStr, Display, EnumString, IntoStaticStr};
use thiserror::Error;

/// Type alias for boxed dynamic errors that can be sent across threads.
pub type BoxedError = Box<dyn std::error::Error + Send + Sync>;

/// Type alias for `Result`s with the webhook [`Error`](struct@Error) type.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Categories of errors that can occur during webhook delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[derive(AsRefStr, Display, EnumString, IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum ErrorKind {
    /// The endpoint URL is malformed or otherwise invalid.
    InvalidEndpoint,
    /// The payload could not be delivered due to a transport failure.
    DeliveryFailed,
    /// The delivery request timed out.
    Timeout,
    /// The endpoint returned a non-retryable failure status.
    NonRetryableStatus,
    /// Computing the HMAC signature for the payload failed.
    SignatureError,
    /// Serializing the webhook payload failed.
    Serialization,
    /// The webhook client is misconfigured.
    Configuration,
    /// An unclassified delivery error occurred.
    #[default]
    Unknown,
}

impl ErrorKind {
    /// Whether a delivery failure of this kind is worth retrying.
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        matches!(self, Self::DeliveryFailed | Self::Timeout)
    }
}

/// Structured error describing a webhook delivery failure.
#[must_use]
#[derive(Debug, Error)]
#[error("[{kind}]{}", message.as_ref().map(|m| format!(": {m}")).unwrap_or_default())]
pub struct Error {
    /// The kind of delivery error that occurred.
    pub kind: ErrorKind,
    /// Primary error message.
    pub message: Option<HipStr<'static>>,
    /// Underlying source error, if any.
    #[source]
    pub source: Option<BoxedError>,
}

impl Error {
    /// Creates a new error with the given kind.
    pub fn new(kind: ErrorKind) -> Self {
        Self {
            kind,
            message: None,
            source: None,
        }
    }

    /// Creates a new error from a source error.
    pub fn from_source(kind: ErrorKind, source: impl Into<BoxedError>) -> Self {
        Self {
            kind,
            message: None,
            source: Some(source.into()),
        }
    }

    /// Adds a message to this error.
    pub fn with_message(mut self, message: impl Into<HipStr<'static>>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Sets the source of the error.
    pub fn with_source(mut self, source: impl Into<BoxedError>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Whether this delivery error is worth retrying.
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        self.kind.is_retryable()
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn test_error_new() {
        let error = Error::new(ErrorKind::Unknown);
        assert_eq!(error.kind, ErrorKind::Unknown);
        assert!(error.message.is_none());
        assert!(error.source.is_none());
    }

    #[test]
    fn test_error_builder_pattern() {
        let error = Error::new(ErrorKind::Configuration).with_message("bad config");

        assert_eq!(error.kind, ErrorKind::Configuration);
        assert_eq!(error.message.as_deref(), Some("bad config"));
    }

    #[test]
    fn test_error_display() {
        let error = Error::new(ErrorKind::DeliveryFailed).with_message("connection refused");

        let display_str = error.to_string();
        assert!(display_str.contains("delivery_failed"));
        assert!(display_str.contains("connection refused"));
    }

    #[test]
    fn test_is_retryable() {
        assert!(Error::new(ErrorKind::DeliveryFailed).is_retryable());
        assert!(Error::new(ErrorKind::Timeout).is_retryable());

        assert!(!Error::new(ErrorKind::InvalidEndpoint).is_retryable());
        assert!(!Error::new(ErrorKind::NonRetryableStatus).is_retryable());
        assert!(!Error::new(ErrorKind::SignatureError).is_retryable());
    }

    #[test]
    fn test_from_source() {
        let source = std::io::Error::other("underlying error");
        let error = Error::from_source(ErrorKind::DeliveryFailed, source);

        assert!(error.source.is_some());
        assert_eq!(error.kind, ErrorKind::DeliveryFailed);
    }

    #[test]
    fn test_default() {
        assert_eq!(ErrorKind::default(), ErrorKind::Unknown);
    }

    #[test]
    fn test_from_str() {
        assert_eq!(
            ErrorKind::from_str("delivery_failed").unwrap(),
            ErrorKind::DeliveryFailed
        );
        assert_eq!(ErrorKind::from_str("timeout").unwrap(), ErrorKind::Timeout);
        assert!(ErrorKind::from_str("invalid").is_err());
    }
}
