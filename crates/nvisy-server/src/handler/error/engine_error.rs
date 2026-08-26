//! Redaction-engine error to HTTP error conversion.
//!
//! Maps `elide_pipeline::Error` onto an HTTP error by its kind: a
//! `MalformedInput` is a bad document the caller supplied (a client error),
//! while every other kind — including `CapabilityUnavailable` (a codec/renderer
//! the build does not ship) — is a server-side processing fault. The engine's
//! own message travels along as context.

use elide_pipeline::ErrorKind as EngineErrorKind;
use elide_pipeline::entity::EditError;

use super::http_error::{Error as HttpError, ErrorKind};

impl<'a> From<elide_pipeline::Error> for HttpError<'a> {
    fn from(error: elide_pipeline::Error) -> Self {
        match error.kind() {
            EngineErrorKind::MalformedInput => ErrorKind::BadRequest
                .with_message("Document could not be processed")
                .with_context(error.to_string()),
            _ => ErrorKind::InternalServerError
                .with_message("Redaction engine failed")
                .with_context(error.to_string()),
        }
    }
}

impl<'a> From<EditError> for HttpError<'a> {
    /// A reviewer edit set that does not apply to the analysis is always a client
    /// error: both an unknown target (a stale or wrong-modality entity id) and a
    /// self-contradiction (two edits deciding one entity differently) are the
    /// caller's edits being wrong, so they map to a 400.
    fn from(error: EditError) -> Self {
        ErrorKind::BadRequest
            .with_message("A reviewer edit does not apply to this analysis")
            .with_context(error.to_string())
    }
}
