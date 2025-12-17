//! Error types and utilities for database operations.
//!
//! This module provides comprehensive error handling for all database operations,
//! including connection errors, query errors, migration errors, and timeout errors.

use std::borrow::Cow;

use deadpool::managed::TimeoutType;
use diesel::result::{ConnectionError, Error};
use diesel_async::pooled_connection::PoolError as DieselPoolError;
use diesel_async::pooled_connection::deadpool::PoolError as DeadpoolError;

use crate::types::ConstraintViolation;

/// Type-erased error type for dynamic error handling.
pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Comprehensive error type for all PostgreSQL database operations.
///
/// This enum covers all possible error conditions that can occur when working
/// with the database, including connection issues, query failures, timeouts,
/// and migration problems.
#[derive(Debug, thiserror::Error)]
#[must_use = "database errors should be handled appropriately"]
pub enum PgError {
    /// Configuration error.
    ///
    /// This includes invalid configuration parameters, missing required settings,
    /// or other issues related to the database configuration.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Database operation timed out.
    ///
    /// This can occur during connection creation, waiting for available connections,
    /// or connection recycling operations.
    #[error("Database operation timed out")]
    Timeout(TimeoutType),

    /// Failed to establish or maintain a database connection.
    ///
    /// This includes authentication failures, network issues, and invalid
    /// connection parameters.
    #[error("Database connection error: {0}")]
    Connection(#[from] ConnectionError),

    /// Database migration operation failed.
    ///
    /// This occurs when applying or rolling back database schema changes.
    #[error("Database migration error: {0}")]
    Migration(BoxError),

    /// Database query execution failed.
    ///
    /// This includes SQL syntax errors, constraint violations, type mismatches,
    /// and other query-related failures.
    #[error("Database query error: {0}")]
    Query(#[from] Error),

    /// Unexpected error occurred.
    ///
    /// This can occur when an error is encountered that is not covered by the
    /// other error types.
    #[error("Unexpected error: {0}")]
    Unexpected(Cow<'static, str>),
}

impl PgError {
    /// Extracts the constraint name from a constraint violation error.
    ///
    /// This is useful for handling specific database constraint violations
    /// and providing user-friendly error messages.
    ///
    /// # Returns
    ///
    /// - `Some(constraint_name)` if this error represents a constraint violation
    /// - `None` if this error is not related to a constraint violation
    pub fn constraint(&self) -> Option<&str> {
        let PgError::Query(err) = self else {
            return None;
        };

        let Error::DatabaseError(_, err) = err else {
            return None;
        };

        err.constraint_name()
    }

    /// Returns a structured constraint violation if this error represents one.
    ///
    /// This provides a more structured way to handle known constraint violations
    /// using the [`ConstraintViolation`] enum.
    ///
    /// # Returns
    ///
    /// - `Some(constraint)` if this error represents a constraint violation
    /// - `None` if this error is not related to a constraint violation
    pub fn constraint_violation(&self) -> Option<ConstraintViolation> {
        self.constraint().and_then(ConstraintViolation::new)
    }

    /// Returns whether this error indicates a transient failure that might succeed on retry.
    ///
    /// Transient errors include timeouts and certain connection issues that may
    /// be resolved by retrying the operation.
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            PgError::Timeout(_) | PgError::Connection(ConnectionError::BadConnection(_))
        )
    }

    /// Returns whether this error indicates a permanent failure that won't succeed on retry.
    ///
    /// Permanent errors include authentication failures, syntax errors, and
    /// constraint violations that require data or schema changes to resolve.
    pub fn is_permanent(&self) -> bool {
        !self.is_transient()
    }
}

impl From<DeadpoolError> for PgError {
    fn from(value: DeadpoolError) -> Self {
        match value {
            DeadpoolError::Timeout(timeout) => Self::Timeout(timeout),
            DeadpoolError::Backend(DieselPoolError::QueryError(error)) => Self::Query(error),
            DeadpoolError::Backend(DieselPoolError::ConnectionError(error)) => {
                Self::Connection(error)
            }
            DeadpoolError::PostCreateHook(err) => {
                // This should not happen with our current hooks, but handle gracefully:
                tracing::warn!("Unexpected post-create hook error: {}", err);
                Self::Unexpected(err.to_string().into())
            }
            DeadpoolError::NoRuntimeSpecified => {
                // This should not happen as we specify tokio runtime, but handle gracefully:
                tracing::error!("No tokio runtime specified for connection pool");
                Self::Unexpected("No runtime specified".into())
            }
            DeadpoolError::Closed => {
                // Pool was closed, treat as connection error:
                Self::Connection(ConnectionError::InvalidConnectionUrl(
                    "Connection pool is closed".into(),
                ))
            }
        }
    }
}

/// Specialized [`Result`] type for database operations.
///
/// This is a convenience alias that uses [`PgError`] as the error type,
/// making database operation signatures cleaner and more consistent.
pub type PgResult<T, E = PgError> = Result<T, E>;
