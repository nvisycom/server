use diesel_async::{AsyncPgConnection, RunQueryDsl};
use tracing::{debug, info, instrument, warn};

use super::MigrationStatus;
use crate::{PgError, PgResult, TRACING_TARGET_MIGRATION};

/// Gets the current migration status of the database.
#[instrument(skip(conn), target = TRACING_TARGET_MIGRATION)]
pub async fn get_migration_status(conn: &mut AsyncPgConnection) -> PgResult<MigrationStatus> {
    debug!(
        target: TRACING_TARGET_MIGRATION,
        "Checking database migration status",
    );

    // Get applied migrations from __diesel_schema_migrations table
    let applied_versions = get_applied_migrations(conn).await?;

    // Get all available migrations - simplified approach for now
    let all_migrations: Vec<String> = vec![]; // TODO: Implement proper migration enumeration

    // Determine pending migrations
    let pending_versions: Vec<String> = all_migrations
        .into_iter()
        .filter(|version| !applied_versions.contains(version))
        .collect();

    let status = MigrationStatus::new(applied_versions, pending_versions);

    debug!(
        target: TRACING_TARGET_MIGRATION,
        applied_count = status.applied_migrations(),
        pending_count = status.pending_migrations(),
        is_up_to_date = status.is_up_to_date(),
        "Migration status retrieved"
    );

    Ok(status)
}

/// Verifies the integrity of the database schema.
#[instrument(skip(conn), target = TRACING_TARGET_MIGRATION)]
pub async fn verify_schema_integrity(conn: &mut AsyncPgConnection) -> PgResult<()> {
    info!(target: TRACING_TARGET_MIGRATION, "Performing database schema integrity verification");

    use diesel::sql_query;

    #[derive(diesel::QueryableByName)]
    struct ExistsResult {
        #[diesel(sql_type = diesel::sql_types::Bool)]
        exists: bool,
    }

    // Check that migration table exists
    let migration_table_exists: bool = sql_query(
        "SELECT EXISTS (
            SELECT FROM information_schema.tables
            WHERE table_name = '__diesel_schema_migrations'
         ) as exists",
    )
    .get_result::<ExistsResult>(conn)
    .await
    .map_err(|e| PgError::Migration(format!("Failed to check migration table: {}", e).into()))?
    .exists;

    if !migration_table_exists {
        warn!(target: TRACING_TARGET_MIGRATION, "Migration table does not exist, database may not be initialized");
        return Err(PgError::Migration(
            "Migration table __diesel_schema_migrations does not exist".into(),
        ));
    }

    // Additional integrity checks could be added here
    // For example: checking for orphaned migration files, validating schema structure, etc.

    info!(target: TRACING_TARGET_MIGRATION, "Database schema integrity verification passed");
    Ok(())
}

/// Gets list of applied migration versions from the database.
#[instrument(skip(conn), target = TRACING_TARGET_MIGRATION)]
pub async fn get_applied_migrations(conn: &mut AsyncPgConnection) -> PgResult<Vec<String>> {
    use diesel::sql_query;

    debug!(
        target: TRACING_TARGET_MIGRATION,
        "Retrieving applied migrations",
    );

    #[derive(diesel::QueryableByName)]
    struct MigrationVersion {
        #[diesel(sql_type = diesel::sql_types::Text)]
        version: String,
    }

    let versions = sql_query("SELECT version FROM __diesel_schema_migrations ORDER BY version")
        .get_results::<MigrationVersion>(conn)
        .await
        .map_err(|e| PgError::Migration(format!("Failed to get applied migrations: {}", e).into()))?
        .into_iter()
        .map(|row| row.version)
        .collect();

    Ok(versions)
}
