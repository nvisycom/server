//! Includes all callbacks and hooks for [`diesel`] and [`deadpool`].

use deadpool::managed::{HookResult, Metrics};
use diesel::ConnectionResult;
use diesel_async::pooled_connection::{PoolError, PoolableConnection};
use diesel_async::{AsyncConnection, AsyncPgConnection};
use futures::FutureExt;
use futures::future::BoxFuture;

use crate::TRACING_TARGET_CONNECTION;

/// Custom setup procedure used to establish a new connection.
///
/// See [`ManagerConfig`] and [`SetupCallback`] for more details.
///
/// [`ManagerConfig`]: diesel_async::pooled_connection::ManagerConfig
/// [`SetupCallback`]: diesel_async::pooled_connection::SetupCallback
pub fn setup_callback<C>(addr: &str) -> BoxFuture<'_, ConnectionResult<C>>
where
    C: AsyncConnection + 'static,
{
    tracing::trace!(
        target: TRACING_TARGET_CONNECTION,
        hook = "setup_callback",
    );

    C::establish(addr).boxed()
}

/// Custom hook called after a new connection has been established.
///
/// See [`PoolBuilder`] for more details.
///
/// [`PoolBuilder`]: deadpool::managed::PoolBuilder
pub fn post_create(conn: &mut AsyncPgConnection, _metrics: &Metrics) -> HookResult<PoolError> {
    tracing::trace!(
        target: TRACING_TARGET_CONNECTION,
        hook = "post_create",
        is_broken = conn.is_broken(),
    );

    // Note: should never return an error.
    Ok(())
}

/// Custom hook called before a connection has been recycled.
///
/// See [`PoolBuilder`] for more details.
///
/// [`PoolBuilder`]: deadpool::managed::PoolBuilder
pub fn pre_recycle(conn: &mut AsyncPgConnection, _metrics: &Metrics) -> HookResult<PoolError> {
    tracing::trace!(
        target: TRACING_TARGET_CONNECTION,
        hook = "pre_recycle",
        is_broken = conn.is_broken(),
    );

    // Note: should never return an error.
    Ok(())
}

/// Custom hook called after a connection has been recycled.
///
/// See [`PoolBuilder`] for more details.
///
/// [`PoolBuilder`]: deadpool::managed::PoolBuilder
pub fn post_recycle(conn: &mut AsyncPgConnection, _metrics: &Metrics) -> HookResult<PoolError> {
    tracing::trace!(
        target: TRACING_TARGET_CONNECTION,
        hook = "post_recycle",
        is_broken = conn.is_broken(),
    );

    // Note: should never return an error.
    Ok(())
}
