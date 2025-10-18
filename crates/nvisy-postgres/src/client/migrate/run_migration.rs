use std::ops::DerefMut;
use std::time::Instant;

use diesel_async::AsyncPgConnection;
use diesel_async::async_connection_wrapper::AsyncConnectionWrapper;
use diesel_migrations::MigrationHarness;
use tokio::task::spawn_blocking;
use tracing::{debug, error, info, instrument};

use super::{MigrationResult, custom_hooks};
use crate::migrate::get_migration_status;
use crate::{MIGRATIONS, PgDatabase, PgError, PgResult, TRACING_TARGET_MIGRATIONS};

/// Run all pending migrations on the database.
#[instrument(skip(pg), target = TRACING_TARGET_MIGRATIONS)]
pub async fn run_pending_migrations(pg: &PgDatabase) -> PgResult<MigrationResult> {
    info!(
        target: TRACING_TARGET_MIGRATIONS,
        "Starting database migration process",
    );

    let start_time = Instant::now();
    let mut conn = pg.get_connection().await?;
    let initial_status = get_migration_status(&mut conn).await?;

    if initial_status.is_up_to_date() {
        info!(
            target: TRACING_TARGET_MIGRATIONS,
            "Database schema is already up to date, no migrations to apply"
        );
        return Ok(MigrationResult::success(start_time.elapsed(), vec![]));
    }

    info!(
        target: TRACING_TARGET_MIGRATIONS,
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
        error!(
            target: TRACING_TARGET_MIGRATIONS,
            duration = ?duration,
            error = %err,
            "Migration task panicked, join error occurred"
        );

        PgError::Migration(err.into())
    })?;

    run_post_migrate_hook(conn.deref_mut()).await?;
    let versions = results.map_err(|err| {
        error!(
            target: TRACING_TARGET_MIGRATIONS,
            duration = ?duration,
            error = &err,
            "Database migration process failed"
        );

        PgError::Migration(err)
    })?;

    info!(
        target: TRACING_TARGET_MIGRATIONS,
        duration = ?duration,
        migrations_count = versions.len(),
        "Database migration process completed successfully"
    );

    Ok(MigrationResult::success(duration, versions))
}

/// Runs the pre-migration hooks.
async fn run_pre_migrate_hook(conn: &mut AsyncPgConnection) -> PgResult<()> {
    debug!(target: TRACING_TARGET_MIGRATIONS, "Executing pre-migration hooks");
    if let Err(e) = custom_hooks::pre_migrate(conn).await {
        error!(target: TRACING_TARGET_MIGRATIONS, error = %e, "Pre-migration hook failed");
        return Err(PgError::Migration(
            format!("Pre-migration hook failed: {}", e).into(),
        ));
    };

    Ok(())
}

/// Runs the post-migration hooks.
async fn run_post_migrate_hook(conn: &mut AsyncPgConnection) -> PgResult<()> {
    debug!(target: TRACING_TARGET_MIGRATIONS, "Executing post-migration hooks");
    if let Err(e) = custom_hooks::post_migrate(conn).await {
        error!(target: TRACING_TARGET_MIGRATIONS, error = %e, "Post-migration hook failed");
        return Err(PgError::Migration(
            format!("Post-migration hook failed: {}", e).into(),
        ));
    };

    Ok(())
}
