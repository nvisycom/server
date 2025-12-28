//! Includes all callbacks and hooks for [`diesel`] and [`deadpool`].

use std::time::Instant;

use deadpool::managed::{HookResult, Metrics};
use diesel::ConnectionResult;
use diesel_async::pooled_connection::{PoolError, PoolableConnection};
use diesel_async::{AsyncConnection, AsyncPgConnection};
use futures::FutureExt;
use futures::future::BoxFuture;

use crate::TRACING_TARGET_CONNECTION;

/// Masks sensitive information (password) in a database URL for safe logging.
fn mask_url(url: &str) -> String {
    if let Some(at_pos) = url.find('@')
        && let Some(colon_pos) = url[..at_pos].rfind(':')
    {
        let mut masked = url.to_string();
        masked.replace_range(colon_pos + 1..at_pos, "***");
        return masked;
    }
    url.to_string()
}

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
    let start = Instant::now();
    let masked_addr = mask_url(addr);

    tracing::info!(
        target: TRACING_TARGET_CONNECTION,
        hook = "setup_callback",
        addr = %masked_addr,
        "Establishing new database connection"
    );

    async move {
        let result = C::establish(addr).await;
        let elapsed = start.elapsed();

        match &result {
            Ok(_) => {
                tracing::info!(
                    target: TRACING_TARGET_CONNECTION,
                    hook = "setup_callback",
                    addr = %masked_addr,
                    elapsed_ms = elapsed.as_millis(),
                    "Database connection established successfully"
                );
            }
            Err(err) => {
                tracing::error!(
                    target: TRACING_TARGET_CONNECTION,
                    hook = "setup_callback",
                    addr = %masked_addr,
                    elapsed_ms = elapsed.as_millis(),
                    error = %err,
                    "Failed to establish database connection"
                );
            }
        }

        result
    }
    .boxed()
}

/// Custom hook called after a new connection has been established.
///
/// See [`PoolBuilder`] for more details.
///
/// [`PoolBuilder`]: deadpool::managed::PoolBuilder
pub fn post_create(conn: &mut AsyncPgConnection, metrics: &Metrics) -> HookResult<PoolError> {
    let is_broken = conn.is_broken();

    tracing::info!(
        target: TRACING_TARGET_CONNECTION,
        hook = "post_create",
        is_broken = is_broken,
        created_at = ?metrics.created,
        recycle_count = metrics.recycle_count,
        "Connection created and added to pool"
    );

    if is_broken {
        tracing::warn!(
            target: TRACING_TARGET_CONNECTION,
            hook = "post_create",
            "Connection is broken after creation"
        );
    }

    // Note: should never return an error.
    Ok(())
}

/// Custom hook called before a connection has been recycled.
///
/// See [`PoolBuilder`] for more details.
///
/// [`PoolBuilder`]: deadpool::managed::PoolBuilder
pub fn pre_recycle(conn: &mut AsyncPgConnection, metrics: &Metrics) -> HookResult<PoolError> {
    let is_broken = conn.is_broken();

    tracing::debug!(
        target: TRACING_TARGET_CONNECTION,
        hook = "pre_recycle",
        is_broken = is_broken,
        created_at = ?metrics.created,
        last_recycled = ?metrics.recycled,
        recycle_count = metrics.recycle_count,
        "Preparing to recycle connection"
    );

    if is_broken {
        tracing::warn!(
            target: TRACING_TARGET_CONNECTION,
            hook = "pre_recycle",
            recycle_count = metrics.recycle_count,
            "Connection is broken before recycling"
        );
    }

    // Note: should never return an error.
    Ok(())
}

/// Custom hook called after a connection has been recycled.
///
/// See [`PoolBuilder`] for more details.
///
/// [`PoolBuilder`]: deadpool::managed::PoolBuilder
pub fn post_recycle(conn: &mut AsyncPgConnection, metrics: &Metrics) -> HookResult<PoolError> {
    let is_broken = conn.is_broken();

    tracing::debug!(
        target: TRACING_TARGET_CONNECTION,
        hook = "post_recycle",
        is_broken = is_broken,
        created_at = ?metrics.created,
        last_recycled = ?metrics.recycled,
        recycle_count = metrics.recycle_count,
        "Connection recycled successfully"
    );

    if is_broken {
        tracing::error!(
            target: TRACING_TARGET_CONNECTION,
            hook = "post_recycle",
            recycle_count = metrics.recycle_count,
            "Connection is broken after recycling, should be removed from pool"
        );
    }

    // Note: should never return an error.
    Ok(())
}
