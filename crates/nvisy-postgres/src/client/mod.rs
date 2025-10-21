//! PostgreSQL client with connection pooling and migration management.
//!
//! This module provides a high-level interface for connecting to PostgreSQL databases,
//! managing connection pools, and handling database migrations. It includes comprehensive
//! error handling, observability through tracing, and production-ready configuration.
//!
//! ## Features
//!
//! - **Connection Pooling**: Efficient connection management with configurable pool settings
//! - **Migration Management**: Automated database schema migrations with rollback support
//! - **Observability**: Comprehensive tracing and metrics for database operations
//! - **Configuration**: Flexible configuration with validation and defaults
//! - **Error Handling**: Rich error types with context and debugging information

pub(crate) mod custom_hooks;
pub mod migrate;
mod pg_database;
mod pool_configs;
mod pool_status;

use deadpool::managed::{Object, Pool};
use diesel_async::AsyncPgConnection;
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
pub use migrate::{MigrationResult, MigrationStatus, PgClientExt};
pub use pg_database::PgClient;
pub use pool_configs::{PgConfig, PgPoolConfig};
pub use pool_status::PoolStatus;

/// Type alias for the connection pool used throughout the application.
pub type ConnectionPool = Pool<AsyncDieselConnectionManager<AsyncPgConnection>>;

/// Type alias for a connection object from the pool.
pub type PooledConnection = Object<AsyncDieselConnectionManager<AsyncPgConnection>>;
