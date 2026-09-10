//! Cloud file-service error to HTTP error conversion.
//!
//! Maps `nvisy_file_service::Error` onto HTTP errors so cloud file-service
//! failures surface with appropriate status codes.

use nvisy_file_service::{Error as FileServiceError, ErrorKind as FileServiceErrorKind};

use super::http_error::{Error as HttpError, ErrorKind};

impl<'a> From<FileServiceError> for HttpError<'a> {
    fn from(error: FileServiceError) -> Self {
        let message = error.to_string();
        match error.kind() {
            FileServiceErrorKind::NotFound => ErrorKind::NotFound
                .with_message("File not found")
                .with_context(message),
            FileServiceErrorKind::PermissionDenied | FileServiceErrorKind::Unauthenticated => {
                ErrorKind::BadRequest
                    .with_message("Cloud file provider rejected the credentials")
                    .with_context(message)
            }
            FileServiceErrorKind::BadRequest => ErrorKind::BadRequest
                .with_message("Cloud file provider rejected the request")
                .with_context(message),
            FileServiceErrorKind::Connection => ErrorKind::ServiceUnavailable
                .with_message("Could not connect to the cloud file provider")
                .with_context(message),
            FileServiceErrorKind::Runtime => ErrorKind::InternalServerError
                .with_message("Cloud file provider operation failed")
                .with_context(message),
        }
    }
}
