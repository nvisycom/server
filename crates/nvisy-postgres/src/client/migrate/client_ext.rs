//! Extension trait for PgClient providing migration functionality.
//!
//! This module provides a clean extension trait that adds migration capabilities
//! to the `PgClient` struct, keeping migration-related functionality separate
//! from the core database client implementation.

use crate::migrate::{
    MigrationResult, MigrationStatus, get_migration_status, run_pending_migrations,
    verify_schema_integrity,
};
use crate::{PgClient, PgResult};

/// Extension trait providing migration functionality for PgClient.
///
/// This trait adds methods for managing database migrations, including
/// applying pending migrations, rolling back changes, and checking
/// migration status.
pub trait PgClientExt {
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
    fn run_pending_migrations(&self) -> impl Future<Output = PgResult<MigrationResult>>;

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
    fn get_migration_status(&self) -> impl Future<Output = PgResult<MigrationStatus>>;

    /// Verifies the integrity of the database schema.
    ///
    /// This method performs basic checks to ensure the database schema is in
    /// a consistent state and that the migration system is properly initialized.
    ///
    /// # Errors
    ///
    /// Returns an error if schema integrity issues are detected or if
    /// verification cannot be completed.
    fn verify_schema_integrity(&self) -> impl Future<Output = PgResult<()>>;
}

impl PgClientExt for PgClient {
    async fn run_pending_migrations(&self) -> PgResult<MigrationResult> {
        run_pending_migrations(self).await
    }

    async fn get_migration_status(&self) -> PgResult<MigrationStatus> {
        let mut conn = self.get_connection().await?;
        get_migration_status(&mut conn).await
    }

    async fn verify_schema_integrity(&self) -> PgResult<()> {
        let mut conn = self.get_connection().await?;
        verify_schema_integrity(&mut conn).await
    }
}
