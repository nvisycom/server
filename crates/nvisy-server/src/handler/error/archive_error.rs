//! Archive error to HTTP error conversion implementation.
//!
//! This module provides conversion from nvisy-runtime archive errors to appropriate
//! HTTP errors with proper status codes and user-friendly messages.

use nvisy_runtime::ArchiveError;

use super::http_error::{Error as HttpError, ErrorKind};

/// Tracing target for archive error conversions.
const TRACING_TARGET: &str = "nvisy_server::handler::archive";

impl From<ArchiveError> for HttpError<'static> {
    fn from(error: ArchiveError) -> Self {
        tracing::error!(
            target: TRACING_TARGET,
            error = %error,
            "Archive operation failed"
        );

        match error {
            ArchiveError::Archive(e) => ErrorKind::InternalServerError
                .with_message("Failed to create archive")
                .with_context(e.to_string()),

            ArchiveError::Io(e) => ErrorKind::InternalServerError
                .with_message("Archive I/O error")
                .with_context(e.to_string()),
        }
    }
}
