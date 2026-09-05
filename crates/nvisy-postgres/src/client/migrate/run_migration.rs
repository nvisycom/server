use std::time::Instant;

use diesel::connection::SimpleConnection;
use diesel::migration::MigrationSource;
use diesel::pg::Pg;
use diesel_async::async_connection_wrapper::AsyncConnectionWrapper;
use diesel_migrations::MigrationHarness;
use tokio::task::spawn_blocking;

use super::MigrationResult;
use crate::{Error, MIGRATIONS, PgClient, Result, TRACING_TARGET_MIGRATION};

/// Session-level advisory-lock key that serializes migration runs across every
/// instance sharing a database.
///
/// `diesel_migrations` does not lock around its check-then-apply (it reads the
/// pending set, then applies), so concurrently-booting replicas would otherwise
/// race on DDL. Holding this lock for the whole run makes the first booter apply
/// migrations while the rest wait, then find nothing pending.
///
/// The value is arbitrary but must stay stable forever: every instance has to
/// contend on the same key. (`0x_6E76_6973_795F_6D69` — "nvisy_mi".) The same
/// key guards every source, so a downstream's [`run_migrations`] serializes
/// against upstream's boot too.
const MIGRATION_LOCK_KEY: i64 = 0x6E76_6973_795F_6D69;

/// Applies this crate's built-in pending migrations, serialized across instances
/// by a Postgres session advisory lock.
///
/// A downstream that embeds its own migration set applies it with
/// [`run_migrations`] after this call (see its ordering note).
#[tracing::instrument(skip(pg), target = TRACING_TARGET_MIGRATION)]
pub async fn run_pending_migrations(pg: &PgClient) -> Result<MigrationResult> {
    run_migrations(pg, MIGRATIONS).await
}

/// Applies all pending migrations from `source`, serialized across instances by
/// a Postgres session advisory lock (the same key as [`run_pending_migrations`]).
///
/// This is the general entry point: [`run_pending_migrations`] calls it with the
/// built-in set, and a downstream binary calls it with its own
/// `embed_migrations!` source to run its migrations through the same locked flow.
///
/// # Ordering
///
/// Each source is applied in its own version order; separate calls do **not**
/// interleave by timestamp. Because upstream (this crate) never depends on a
/// downstream migration, the only requirement is that a downstream migration's
/// upstream dependencies are already applied — so a downstream applies its set
/// *after* [`run_pending_migrations`]. Diesel imposes no timestamp monotonicity,
/// so a later-added, earlier-timestamped migration still applies cleanly; only
/// the recorded history order reflects when it ran.
#[tracing::instrument(skip(pg, source), target = TRACING_TARGET_MIGRATION)]
pub async fn run_migrations<S>(pg: &PgClient, source: S) -> Result<MigrationResult>
where
    S: MigrationSource<Pg> + Send + 'static,
{
    tracing::info!(target: TRACING_TARGET_MIGRATION, "Starting database migration process");
    let start_time = Instant::now();

    // The migration harness is synchronous, so the whole run — acquire lock,
    // apply migrations, release lock — happens on one blocking connection.
    let conn: AsyncConnectionWrapper<_> = pg.get_pooled_connection().await?.into();
    let versions = spawn_blocking(move || migrate_locked(conn, source))
        .await
        .map_err(|err| {
            tracing::error!(target: TRACING_TARGET_MIGRATION, error = %err, "Migration task panicked");
            Error::Migration(err.into())
        })??;

    let duration = start_time.elapsed();
    tracing::info!(
        target: TRACING_TARGET_MIGRATION,
        duration = ?duration,
        migrations_count = versions.len(),
        "Database migration process completed successfully"
    );
    Ok(MigrationResult::success(duration, versions))
}

/// Runs `source`'s pending migrations while holding the advisory lock, on a
/// blocking connection. The lock is released before returning, on every path, by
/// the [`AdvisoryLock`] guard's `Drop`.
fn migrate_locked<C, S>(mut conn: AsyncConnectionWrapper<C>, source: S) -> Result<Vec<String>>
where
    AsyncConnectionWrapper<C>: diesel::Connection + MigrationHarness<Pg>,
    S: MigrationSource<Pg>,
{
    let mut lock = AdvisoryLock::acquire(&mut conn, MIGRATION_LOCK_KEY)?;

    let versions = lock
        .connection()
        .run_pending_migrations(source)
        .map_err(|err| {
            tracing::error!(target: TRACING_TARGET_MIGRATION, error = &*err, "Database migration failed");
            Error::Migration(err)
        })?
        .into_iter()
        .map(|version| version.to_string())
        .collect();

    Ok(versions)
    // `lock` drops here, releasing the advisory lock.
}

/// RAII guard for a Postgres session advisory lock held on a blocking
/// connection: acquired on construction, released on `Drop` (including on an
/// error or panic during the migration run). Work done while holding the lock
/// goes through [`connection`](Self::connection).
struct AdvisoryLock<'conn, C: SimpleConnection> {
    conn: &'conn mut C,
    key: i64,
}

impl<'conn, C: SimpleConnection> AdvisoryLock<'conn, C> {
    /// Blocks until the session advisory lock for `key` is held.
    fn acquire(conn: &'conn mut C, key: i64) -> Result<Self> {
        conn.batch_execute(&format!("SELECT pg_advisory_lock({key})"))
            .map_err(|err| {
                Error::Migration(format!("failed to acquire migration lock: {err}").into())
            })?;
        Ok(Self { conn, key })
    }

    /// The locked connection, for running work while the lock is held.
    fn connection(&mut self) -> &mut C {
        self.conn
    }
}

impl<C: SimpleConnection> Drop for AdvisoryLock<'_, C> {
    fn drop(&mut self) {
        // Releasing must not mask a migration error, so a failure here is logged
        // rather than propagated; the lock is session-scoped and clears when the
        // connection is eventually recycled even if this call is lost.
        if let Err(err) = self
            .conn
            .batch_execute(&format!("SELECT pg_advisory_unlock({})", self.key))
        {
            tracing::error!(
                target: TRACING_TARGET_MIGRATION,
                error = %err,
                "Failed to release migration advisory lock",
            );
        }
    }
}
