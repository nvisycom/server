//! Shared external-system error to HTTP error conversion.
//!
//! Maps `nvisy_core::Error`, which the object-store, cloud file-service, and
//! webhook clients all return, onto HTTP errors. The user-facing message is the
//! kind's safe reason; the full error is kept as context.

use nvisy_core::{Error as CoreError, ErrorKind as CoreErrorKind};

use super::http_error::{Error as HttpError, ErrorKind};

impl From<CoreError> for HttpError {
    fn from(error: CoreError) -> Self {
        let kind = match error.kind() {
            // The tenant's endpoint, credentials, or request were rejected:
            // something the caller can fix, not this server's auth.
            CoreErrorKind::Invalid
            | CoreErrorKind::NotSupported
            | CoreErrorKind::Unauthenticated
            | CoreErrorKind::PermissionDenied => ErrorKind::BadRequest,
            CoreErrorKind::NotFound => ErrorKind::NotFound,
            CoreErrorKind::AlreadyExists | CoreErrorKind::Precondition => ErrorKind::Conflict,
            CoreErrorKind::Timeout | CoreErrorKind::Connection => ErrorKind::ServiceUnavailable,
            CoreErrorKind::Runtime | CoreErrorKind::Internal => ErrorKind::InternalServerError,
        };
        kind.with_message(error.kind().reason())
            .with_context(error.to_string())
    }
}
