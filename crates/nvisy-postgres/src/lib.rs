#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

/// Embeds all migrations into the final binary.
pub(crate) const MIGRATIONS: diesel_migrations::EmbeddedMigrations =
    diesel_migrations::embed_migrations!();

// Tracing target constants for consistent logging.

/// Tracing target for client-related operations.
///
/// Use this target for logging client initialization, configuration, and lifecycle events.
pub const TRACING_TARGET_CLIENT: &str = "nvisy_postgres::client";

/// Tracing target for database query operations.
///
/// Use this target for logging query execution, results, and query-related errors.
pub const TRACING_TARGET_QUERY: &str = "nvisy_postgres::queries";

/// Tracing target for database migration operations.
///
/// Use this target for logging migration application, rollback, and migration status checks.
pub const TRACING_TARGET_MIGRATION: &str = "nvisy_postgres::migrations";

/// Tracing target for database connection operations.
///
/// Use this target for logging connection establishment, pool management, and connection errors.
pub const TRACING_TARGET_CONNECTION: &str = "nvisy_postgres::connection";

mod client;
pub mod model;
pub mod query;
mod schema;
pub mod types;

use std::borrow::Cow;

use deadpool::managed::TimeoutType;
use diesel::ConnectionError;
use diesel::result::Error;
pub use diesel_async::AsyncPgConnection as PgConnection;

pub use crate::client::{
    ConnectionPool, MigrationResult, MigrationStatus, PgClient, PgClientExt, PgConfig,
    PgPoolConfig, PgPoolStatus, PooledConnection, get_applied_migrations, get_migration_status,
    run_pending_migrations, verify_schema_integrity,
};
use crate::types::ConstraintViolation;

pub mod error {
    //! Error types and utilities for database operations.
    //!
    //! This module provides comprehensive error handling for all database operations,
    //! including connection errors, query errors, migration errors, and timeout errors.
    //!
    //! See [`PgError`] for the main error type used throughout this crate.
    //!
    //! [`PgError`]: crate::PgError

    /// Type-erased error type for dynamic error handling.
    pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

    use std::borrow::Cow;

    pub use deadpool::managed::TimeoutType;
    pub use diesel::result::{ConnectionError as DieselConnectionError, Error as DieselError};
    pub use diesel_async::pooled_connection::PoolError as DieselPoolError;
    pub use diesel_async::pooled_connection::deadpool::PoolError as DeadpoolError;

    /// Provides contextual hints for error types to aid in debugging and user messaging.
    ///
    /// This trait allows error types to provide additional context about what went wrong
    /// and potential remediation steps.
    pub trait ErrorHint {
        /// Returns an additional hint for an error type.
        ///
        /// The hint should provide actionable information about the error context
        /// or potential solutions.
        fn hint(&self) -> Cow<'static, str>;
    }

    impl ErrorHint for TimeoutType {
        fn hint(&self) -> Cow<'static, str> {
            match self {
                TimeoutType::Wait => Cow::Borrowed(
                    "Connection pool is exhausted, consider increasing pool size or optimizing query performance",
                ),
                TimeoutType::Create => Cow::Borrowed(
                    "Unable to establish new database connection, check connection string and database availability",
                ),
                TimeoutType::Recycle => Cow::Borrowed(
                    "Failed to recycle database connection, connection may be in invalid state",
                ),
            }
        }
    }
}

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
    Migration(error::BoxError),

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

impl From<error::DeadpoolError> for PgError {
    fn from(value: error::DeadpoolError) -> Self {
        use error::{DeadpoolError, DieselPoolError};

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
