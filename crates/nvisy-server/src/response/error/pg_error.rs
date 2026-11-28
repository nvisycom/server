//! Constraint violation to HTTP error conversion handlers.
//!
//! This module provides organized handlers for converting `PostgreSQL` constraint
//! violations into appropriate HTTP error responses. Each submodule handles
//! constraints for a specific domain (accounts, workspaces, documents, etc.).
//!
//! All conversions are implemented via the `From` trait for ergonomic usage.

use nvisy_postgres::types::ConstraintViolation;
use nvisy_postgres::{Error as PgError, TimeoutType};

use super::{Error, ErrorKind};

impl From<ConstraintViolation> for Error {
    fn from(constraint: ConstraintViolation) -> Self {
        match constraint {
            ConstraintViolation::Account(c) => c.into(),
            ConstraintViolation::AccountIdentity(c) => c.into(),
            ConstraintViolation::AccountNotification(c) => c.into(),
            ConstraintViolation::AccountApiToken(c) => c.into(),
            ConstraintViolation::Workspace(c) => c.into(),
            ConstraintViolation::WorkspaceMember(c) => c.into(),
            ConstraintViolation::WorkspaceInvite(c) => c.into(),
            ConstraintViolation::WorkspaceActivityLog(c) => c.into(),
            ConstraintViolation::WorkspaceWebhook(c) => c.into(),
            ConstraintViolation::WorkspaceDocument(c) => c.into(),
            ConstraintViolation::WorkspaceBlob(c) => c.into(),
            ConstraintViolation::WorkspaceReview(c) => c.into(),
            ConstraintViolation::WorkspaceReviewComment(c) => c.into(),
            ConstraintViolation::WorkspaceDetection(c) => c.into(),
            ConstraintViolation::WorkspaceConnection(c) => c.into(),
            ConstraintViolation::WorkspaceConnectionSync(c) => c.into(),
            ConstraintViolation::WorkspacePolicy(c) => c.into(),
            ConstraintViolation::WorkspaceProvider(c) => c.into(),
        }
    }
}

impl From<PgError> for Error {
    fn from(error: PgError) -> Self {
        let internal = |detail: String| ErrorKind::InternalServerError.with_context(detail);
        match error {
            // A wait timeout means the pool was saturated: the service is
            // momentarily overloaded, not broken, so surface a retryable 503.
            PgError::Timeout(TimeoutType::Wait) => {
                ErrorKind::ServiceUnavailable.with_message("The server is busy; retry shortly")
            }
            // Any other transient failure (a create/recycle timeout, a lost
            // connection) means the database is not answering: also a retryable
            // 503, which callers and workers can classify.
            ref transient if transient.is_transient() => ErrorKind::ServiceUnavailable
                .with_message("The database is unavailable; retry shortly")
                .with_context(transient.to_string()),
            PgError::Query(_) => {
                // A query that expected a row but matched none (e.g. a
                // `get_result` scoped to a live row that was concurrently
                // tombstoned) is a not-found, not a server error.
                if error.is_not_found() {
                    return ErrorKind::NotFound.into_error();
                }
                match error.constraint().and_then(ConstraintViolation::new) {
                    Some(constraint) => constraint.into(),
                    None => internal(error.to_string()),
                }
            }
            error => internal(error.to_string()),
        }
    }
}

// Used only for transactions.
impl From<nvisy_postgres::DieselError> for Error {
    fn from(error: nvisy_postgres::DieselError) -> Self {
        // Convert DieselError -> PgError -> Error
        let pg_error: PgError = error.into();
        pg_error.into()
    }
}
