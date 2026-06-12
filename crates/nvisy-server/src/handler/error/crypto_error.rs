//! Crypto error to HTTP error conversion.
//!
//! Converts a [`CryptoError`] into the handler HTTP error. Encryption and
//! decryption failures are internal faults, so they map to a 500 with the
//! crypto error attached as context.

use super::http_error::{Error as HttpError, ErrorKind};
use crate::service::crypto::CryptoError;

/// Tracing target for crypto error conversions.
const TRACING_TARGET: &str = "nvisy_server::handler::crypto";

impl From<CryptoError> for HttpError<'static> {
    fn from(error: CryptoError) -> Self {
        tracing::error!(
            target: TRACING_TARGET,
            error = %error,
            "Cryptographic operation failed"
        );

        ErrorKind::InternalServerError
            .with_message("Cryptographic operation failed")
            .with_context(error.to_string())
    }
}
