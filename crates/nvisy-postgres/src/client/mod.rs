//! PostgreSQL client with connection pooling and migration management.
//!
//! This module provides a high-level interface for connecting to PostgreSQL databases,
//! managing connection pools, and handling database migrations. It includes comprehensive
//! error handling, observability through tracing, and production-ready configuration.

pub(crate) mod custom_hooks;
mod health;
pub mod migrate;
mod pg_client;
mod pg_config;

use deadpool::managed::{Object, Pool};
use diesel_async::AsyncPgConnection;
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
pub use migrate::{MigrationResult, MigrationStatus, PgClientMigrationExt};
pub use pg_client::{PgClient, PgConn, PgPoolStatus};
pub use pg_config::PgConfig;

/// Type alias for the connection pool used throughout the application.
pub type ConnectionPool = Pool<AsyncDieselConnectionManager<AsyncPgConnection>>;

/// Type alias for a connection object from the pool.
pub type PooledConnection = Object<AsyncDieselConnectionManager<AsyncPgConnection>>;
