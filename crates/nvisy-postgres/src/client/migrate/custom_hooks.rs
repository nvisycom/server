//! Includes all callbacks and hooks for [`PgDatabaseExt`].
//!
//! [`PgDatabaseExt`]: super::PgDatabaseExt

use diesel_async::AsyncPgConnection;
use diesel_async::pooled_connection::PoolableConnection;

use crate::{PgResult, TRACING_TARGET_MIGRATIONS};

/// Custom hook called before a connection has been used to run migrations.
///
/// See [`PgDatabaseExt`] for more details.
///
/// [`PgDatabaseExt`]: super::PgDatabaseExt
pub async fn pre_migrate(conn: &mut AsyncPgConnection) -> PgResult<()> {
    tracing::trace!(
        target: TRACING_TARGET_MIGRATIONS,
        hook = "pre_migrate",
        is_broken = conn.is_broken(),
    );

    Ok(())
}

/// Custom hook called after a connection has been used to run migrations.
///
/// See [`PgDatabaseExt`] for more details.
///
/// [`PgDatabaseExt`]: super::PgDatabaseExt
pub async fn post_migrate(conn: &mut AsyncPgConnection) -> PgResult<()> {
    tracing::trace!(
        target: TRACING_TARGET_MIGRATIONS,
        hook = "post_migrate",
        is_broken = conn.is_broken(),
    );

    Ok(())
}
