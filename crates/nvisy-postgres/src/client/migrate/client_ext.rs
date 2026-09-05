//! Extension trait for PgClient providing migration functionality.
//!
//! This module provides a clean extension trait that adds migration capabilities
//! to the `PgClient` struct, keeping migration-related functionality separate
//! from the core database client implementation.

use diesel::migration::MigrationSource;
use diesel::pg::Pg;

use super::{
    MigrationResult, MigrationStatus, get_migration_status, run_migrations, run_pending_migrations,
    verify_schema_integrity,
};
use crate::{PgClient, Result};

/// Extension trait providing migration functionality for PgClient.
///
/// This trait adds methods for managing database migrations, including
/// applying pending migrations, rolling back changes, and checking
/// migration status.
pub trait PgClientMigrationExt {
    /// Runs all pending database migrations.
    ///
    /// This method will apply any unapplied migrations to bring the database schema
    /// up to date. It's safe to call this method multiple times.
    ///
    /// # Returns
    ///
    /// Returns a `MigrationResult` containing information about the migration process,
    /// including the number of migrations processed and their execution time.
    ///
    /// # Errors
    ///
    /// Returns an error if any migration fails to apply or if there are
    /// connectivity issues with the database.
    fn run_pending_migrations(&self) -> impl Future<Output = Result<MigrationResult>>;

    /// Runs all pending migrations from a caller-supplied `source`, through the
    /// same advisory-locked flow as [`run_pending_migrations`](Self::run_pending_migrations).
    ///
    /// A downstream binary uses this for its own `embed_migrations!` set. Apply
    /// it *after* [`run_pending_migrations`](Self::run_pending_migrations): sources
    /// are applied in call order and do not interleave, and downstream migrations
    /// may depend on upstream schema but never the reverse.
    fn run_migrations<S>(&self, source: S) -> impl Future<Output = Result<MigrationResult>>
    where
        S: MigrationSource<Pg> + Send + 'static;

    /// Gets the current migration status of the database.
    ///
    /// This method provides detailed information about which migrations have been
    /// applied and which are pending, useful for monitoring and debugging purposes.
    ///
    /// # Returns
    ///
    /// Returns a `MigrationStatus` struct containing comprehensive information
    /// about the current state of database migrations.
    ///
    /// # Errors
    ///
    /// Returns an error if there are connectivity issues or if the migration
    /// table cannot be accessed.
    fn get_migration_status(&self) -> impl Future<Output = Result<MigrationStatus>>;

    /// Verifies the integrity of the database schema.
    ///
    /// This method performs basic checks to ensure the database schema is in
    /// a consistent state and that the migration system is properly initialized.
    ///
    /// # Errors
    ///
    /// Returns an error if schema integrity issues are detected or if
    /// verification cannot be completed.
    fn verify_schema_integrity(&self) -> impl Future<Output = Result<()>>;
}

impl PgClientMigrationExt for PgClient {
    async fn run_pending_migrations(&self) -> Result<MigrationResult> {
        run_pending_migrations(self).await
    }

    async fn run_migrations<S>(&self, source: S) -> Result<MigrationResult>
    where
        S: MigrationSource<Pg> + Send + 'static,
    {
        run_migrations(self, source).await
    }

    async fn get_migration_status(&self) -> Result<MigrationStatus> {
        let mut conn = self.get_pooled_connection().await?;
        get_migration_status(&mut conn).await
    }

    async fn verify_schema_integrity(&self) -> Result<()> {
        let mut conn = self.get_pooled_connection().await?;
        verify_schema_integrity(&mut conn).await
    }
}
