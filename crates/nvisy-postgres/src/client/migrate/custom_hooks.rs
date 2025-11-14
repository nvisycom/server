//! Includes all callbacks and hooks for [`PgDatabaseExt`].
//!
//! [`PgDatabaseExt`]: super::PgDatabaseExt

use std::time::Instant;

use diesel_async::AsyncPgConnection;
use diesel_async::pooled_connection::PoolableConnection;

use crate::{PgResult, TRACING_TARGET_MIGRATION};

/// Custom hook called before a connection has been used to run migrations.
///
/// See [`PgDatabaseExt`] for more details.
///
/// [`PgDatabaseExt`]: super::PgDatabaseExt
pub async fn pre_migrate(conn: &mut AsyncPgConnection) -> PgResult<()> {
    let is_broken = conn.is_broken();

    tracing::info!(
        target: TRACING_TARGET_MIGRATION,
        hook = "pre_migrate",
        is_broken = is_broken,
        timestamp = ?Instant::now(),
        "Preparing to run database migrations"
    );

    if is_broken {
        tracing::error!(
            target: TRACING_TARGET_MIGRATION,
            hook = "pre_migrate",
            "Connection is broken before migrations - migrations may fail"
        );
    }

    Ok(())
}

/// Custom hook called after a connection has been used to run migrations.
///
/// See [`PgDatabaseExt`] for more details.
///
/// [`PgDatabaseExt`]: super::PgDatabaseExt
pub async fn post_migrate(conn: &mut AsyncPgConnection) -> PgResult<()> {
    let is_broken = conn.is_broken();

    tracing::info!(
        target: TRACING_TARGET_MIGRATION,
        hook = "post_migrate",
        is_broken = is_broken,
        timestamp = ?Instant::now(),
        "Database migrations completed"
    );

    if is_broken {
        tracing::error!(
            target: TRACING_TARGET_MIGRATION,
            hook = "post_migrate",
            "Connection is broken after migrations - possible migration failure"
        );
    }

    Ok(())
}
