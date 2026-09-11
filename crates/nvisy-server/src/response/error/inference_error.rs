//! Inference error to HTTP error conversion.
//!
//! Maps `nvisy_inference::Error` onto an HTTP error. Building the provider
//! client or a completion failing at runtime is a server-side fault (the stored
//! connection is the operator's config, not the caller's input), so it surfaces
//! as an internal error with the provider's own message as context.

use super::http_error::{Error as HttpError, ErrorKind};

impl<'a> From<nvisy_inference::Error> for HttpError<'a> {
    fn from(error: nvisy_inference::Error) -> Self {
        ErrorKind::InternalServerError
            .with_message("Language model request failed")
            .with_context(error.to_string())
    }
}
