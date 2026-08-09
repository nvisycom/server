//! Object-store error to HTTP error conversion.
//!
//! Maps `nvisy_object::Error` onto HTTP errors so object-storage failures
//! surface with appropriate status codes.

use nvisy_object::{Error as ObjectError, ErrorKind as ObjectErrorKind};

use super::http_error::{Error as HttpError, ErrorKind};

impl<'a> From<ObjectError> for HttpError<'a> {
    fn from(error: ObjectError) -> Self {
        let message = error.to_string();
        match error.kind() {
            ObjectErrorKind::NotFound => ErrorKind::NotFound
                .with_message("Object not found")
                .with_context(message),
            ObjectErrorKind::PermissionDenied | ObjectErrorKind::Unauthenticated => {
                ErrorKind::BadRequest
                    .with_message("Object store rejected the credentials")
                    .with_context(message)
            }
            ObjectErrorKind::AlreadyExists => ErrorKind::Conflict
                .with_message("Object already exists")
                .with_context(message),
            ObjectErrorKind::Connection => ErrorKind::BadRequest
                .with_message("Could not connect to the object store")
                .with_context(message),
            _ => ErrorKind::InternalServerError
                .with_message("Object store operation failed")
                .with_context(message),
        }
    }
}
