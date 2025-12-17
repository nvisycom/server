//! Prelude module for nvisy-postgres.
//!
//! This module re-exports the most commonly used types and traits from nvisy-postgres,
//! making it easy to import everything you need with a single `use` statement.
//!
//! # Example
//!
//! ```rust
//! use nvisy_postgres::prelude::*;
//!
//! # async fn example() -> PgResult<()> {
//! let config = PgConfig::from_url("postgresql://localhost/mydb")?;
//! let client = PgClient::new(config).await?;
//! # Ok(())
//! # }
//! ```

// Client types
// Common query traits
pub use diesel::prelude::*;
pub use diesel_async::RunQueryDsl;

// Connection type
pub use crate::PgConnection;
pub use crate::client::{
    ConnectionPool, MigrationResult, MigrationStatus, PgClient, PgClientMigrationExt, PgConfig,
    PgPoolStatus,
};
// Error types
pub use crate::{PgError, PgResult};
