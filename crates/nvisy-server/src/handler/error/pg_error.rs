//! Constraint violation to HTTP error conversion handlers.
//!
//! This module provides organized handlers for converting PostgreSQL constraint
//! violations into appropriate HTTP error responses. Each submodule handles
//! constraints for a specific domain (accounts, projects, documents, etc.).
//!
//! All conversions are implemented via the `From` trait for ergonomic usage.

use nvisy_postgres::PgError;
use nvisy_postgres::types::ConstraintViolation;

use crate::handler::{Error, ErrorKind};

/// Tracing target for account operations.
const TRACING_TARGET: &str = "nvisy_server::postgres_constraints";

impl From<ConstraintViolation> for Error<'static> {
    fn from(constraint: ConstraintViolation) -> Self {
        match constraint {
            ConstraintViolation::Account(c) => c.into(),
            ConstraintViolation::AccountNotification(c) => c.into(),
            ConstraintViolation::AccountApiToken(c) => c.into(),
            ConstraintViolation::AccountActionToken(c) => c.into(),
            ConstraintViolation::Project(c) => c.into(),
            ConstraintViolation::ProjectMember(c) => c.into(),
            ConstraintViolation::ProjectInvite(c) => c.into(),
            ConstraintViolation::ProjectActivityLog(c) => c.into(),
            ConstraintViolation::ProjectIntegration(c) => c.into(),
            ConstraintViolation::ProjectRun(c) => c.into(),
            ConstraintViolation::Document(c) => c.into(),
            ConstraintViolation::DocumentComment(c) => c.into(),
            ConstraintViolation::DocumentAnnotation(c) => c.into(),
            ConstraintViolation::DocumentFile(c) => c.into(),
            ConstraintViolation::DocumentVersion(c) => c.into(),
        }
    }
}

impl From<PgError> for Error<'static> {
    fn from(error: PgError) -> Self {
        match error {
            PgError::Config(config_error) => {
                tracing::error!(
                    target: TRACING_TARGET,
                    error = %config_error,
                    "database configuration error"
                );
                ErrorKind::InternalServerError.into_error()
            }
            PgError::Timeout(timeout) => {
                tracing::error!(
                    target: TRACING_TARGET,
                    timeout = ?timeout,
                    "database timeout",
                );
                ErrorKind::InternalServerError.into_error()
            }
            PgError::Connection(connection_error) => {
                tracing::error!(
                    target: TRACING_TARGET,
                    error = %connection_error,
                    "database connection error"
                );
                ErrorKind::InternalServerError.into_error()
            }
            PgError::Migration(migration_error) => {
                tracing::error!(
                    target: TRACING_TARGET,
                    error = %migration_error,
                    "database migration error"
                );
                ErrorKind::InternalServerError.into_error()
            }
            PgError::Query(ref query_error) => {
                // Try to extract constraint violation
                if let Some(constraint_name) = error.constraint()
                    && let Some(constraint) = ConstraintViolation::new(constraint_name)
                {
                    tracing::error!(
                        target: TRACING_TARGET,
                        constraint = constraint_name,
                        error = %query_error,
                        "query error (constraint violation)"
                    );
                    return constraint.into();
                }

                // Generic query error without constraint
                tracing::error!(
                    target: TRACING_TARGET,
                    error = %query_error,
                    "query error"
                );
                ErrorKind::InternalServerError.into_error()
            }
            PgError::Unexpected(unexpected_error) => {
                tracing::error!(
                    target: TRACING_TARGET,
                    error = %unexpected_error,
                    "unexpected database error"
                );
                ErrorKind::InternalServerError.into_error()
            }
        }
    }
}
