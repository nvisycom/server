#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

/// Embeds all migrations into the final binary.
///
/// Points at the single canonical `migrations/` directory at the repository root
/// (resolved from this crate's manifest directory), so there is one source of
/// truth for migrations rather than a copy kept in sync inside the crate.
pub(crate) const MIGRATIONS: diesel_migrations::EmbeddedMigrations =
    diesel_migrations::embed_migrations!("../../migrations");

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
/// Use this target for logging connection establishment, pool management, client initialization,
/// configuration, and connection errors.
pub const TRACING_TARGET_CONNECTION: &str = "nvisy_postgres::connection";

mod client;
mod error;
pub mod model;
pub mod query;
mod schema;
pub mod types;

pub use diesel_async::{AsyncConnection, AsyncPgConnection as PgConnection};
pub use jiff_diesel::Timestamp as JiffTimestamp;

pub(crate) use crate::client::PooledConnection;
pub use crate::client::{
    ConnectionPool, MigrationResult, MigrationStatus, PgClient, PgClientMigrationExt, PgConfig,
    PgConn, PgPoolStatus,
};
pub use crate::error::{DieselError, Error, Result, TimeoutType};
