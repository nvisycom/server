//! Redaction-engine error to HTTP error conversion.
//!
//! Maps `elide_pipeline::Error` onto an HTTP error. An analyze/anonymize failure
//! is a server-side processing fault, so it surfaces as an internal error with
//! the engine's own message as context.

use super::http_error::{Error as HttpError, ErrorKind};

impl<'a> From<elide_pipeline::Error> for HttpError<'a> {
    fn from(error: elide_pipeline::Error) -> Self {
        ErrorKind::InternalServerError
            .with_message("Redaction engine failed")
            .with_context(error.to_string())
    }
}
