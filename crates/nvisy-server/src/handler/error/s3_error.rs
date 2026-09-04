//! Blob-store error to HTTP error conversion.
//!
//! Every [`S3Error`](nvisy_s3::S3Error) is an infrastructure failure of the
//! first-party object store, invisible to the caller's request shape, so all map
//! to a `500` with a generic message; the specific cause is carried in context
//! for the logs.

use nvisy_s3::S3Error;

use super::http_error::{Error as HttpError, ErrorKind};

impl<'a> From<S3Error> for HttpError<'a> {
    fn from(error: S3Error) -> Self {
        ErrorKind::InternalServerError
            .with_message("Object storage operation failed")
            .with_context(error.to_string())
    }
}
