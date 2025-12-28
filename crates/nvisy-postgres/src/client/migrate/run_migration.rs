use std::ops::DerefMut;
use std::time::Instant;

use diesel_async::AsyncPgConnection;
use diesel_async::async_connection_wrapper::AsyncConnectionWrapper;
use diesel_migrations::MigrationHarness;
use tokio::task::spawn_blocking;

use super::{MigrationResult, custom_hooks, get_migration_status};
use crate::{MIGRATIONS, PgClient, PgError, PgResult, TRACING_TARGET_MIGRATION};

/// Run all pending migrations on the database.
#[tracing::instrument(skip(pg), target = TRACING_TARGET_MIGRATION)]
pub async fn run_pending_migrations(pg: &PgClient) -> PgResult<MigrationResult> {
    tracing::info!(
        target: TRACING_TARGET_MIGRATION,
        "Starting database migration process",
    );

    let start_time = Instant::now();
    let mut conn = pg.get_pooled_connection().await?;
    let initial_status = get_migration_status(&mut conn).await?;

    if initial_status.is_up_to_date() {
        tracing::info!(
            target: TRACING_TARGET_MIGRATION,
            "Database schema is already up to date, no migrations to apply"
        );
        return Ok(MigrationResult::success(start_time.elapsed(), vec![]));
    }

    tracing::info!(
        target: TRACING_TARGET_MIGRATION,
        pending_migrations = initial_status.pending_migrations(),
        "Found pending migrations to apply"
    );

    run_pre_migrate_hook(&mut conn).await?;
    let mut conn: AsyncConnectionWrapper<_> = conn.into();
    let results = spawn_blocking(move || match conn.run_pending_migrations(MIGRATIONS) {
        Ok(versions) => (
            Ok(versions
                .into_iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()),
            conn,
        ),
        Err(x) => (Err(x), conn),
    })
    .await;

    let duration = start_time.elapsed();
    let (results, mut conn) = results.map_err(|err| {
        tracing::error!(
            target: TRACING_TARGET_MIGRATION,
            duration = ?duration,
            error = %err,
            "Migration task panicked, join error occurred"
        );

        PgError::Migration(err.into())
    })?;

    run_post_migrate_hook(conn.deref_mut()).await?;
    let versions = results.map_err(|err| {
        tracing::error!(
            target: TRACING_TARGET_MIGRATION,
            duration = ?duration,
            error = &err,
            "Database migration process failed"
        );

        PgError::Migration(err)
    })?;

    tracing::info!(
        target: TRACING_TARGET_MIGRATION,
        duration = ?duration,
        migrations_count = versions.len(),
        "Database migration process completed successfully"
    );

    Ok(MigrationResult::success(duration, versions))
}

/// Runs the pre-migration hooks.
async fn run_pre_migrate_hook(conn: &mut AsyncPgConnection) -> PgResult<()> {
    tracing::debug!(target: TRACING_TARGET_MIGRATION, "Executing pre-migration hooks");
    if let Err(e) = custom_hooks::pre_migrate(conn).await {
        tracing::error!(target: TRACING_TARGET_MIGRATION, error = %e, "Pre-migration hook failed");
        return Err(PgError::Migration(
            format!("Pre-migration hook failed: {}", e).into(),
        ));
    };

    Ok(())
}

/// Runs the post-migration hooks.
async fn run_post_migrate_hook(conn: &mut AsyncPgConnection) -> PgResult<()> {
    tracing::debug!(target: TRACING_TARGET_MIGRATION, "Executing post-migration hooks");
    if let Err(e) = custom_hooks::post_migrate(conn).await {
        tracing::error!(target: TRACING_TARGET_MIGRATION, error = %e, "Post-migration hook failed");
        return Err(PgError::Migration(
            format!("Post-migration hook failed: {}", e).into(),
        ));
    };

    Ok(())
}
