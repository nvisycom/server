use std::sync::Arc;
use std::time::Duration;

use deadpool::managed::{Hook, Pool};
use diesel_async::RunQueryDsl;
use diesel_async::pooled_connection::{AsyncDieselConnectionManager, ManagerConfig};
use tracing::{debug, error, info, instrument, warn};

use super::custom_hooks;
use crate::{
    ConnectionPool, PgConfig, PgError, PgPoolStatus, PgResult, PooledConnection,
    TRACING_TARGET_CLIENT, TRACING_TARGET_CONNECTION,
};

/// High-level database client that manages connections and migrations.
///
/// This struct provides the main interface for database operations, encapsulating
/// connection pool management, configuration, and migration handling.
#[derive(Clone)]
pub struct PgClient {
    inner: Arc<PgClientInner>,
}

/// Inner data for PgClient
struct PgClientInner {
    pool: ConnectionPool,
    config: PgConfig,
}

impl PgClient {
    /// Creates a new database client with the provided configuration.
    ///
    /// This will establish a connection pool.
    ///
    /// # Arguments
    ///
    /// * `config` - Database configuration including connection details and pool settings
    ///
    /// # Errors
    ///
    /// Returns an error if:
    ///
    /// - Pool configuration is invalid
    #[instrument(skip(config), target = TRACING_TARGET_CLIENT, fields(database_url = %config.database_url_masked()))]
    pub fn new(config: PgConfig) -> PgResult<Self> {
        info!(target: TRACING_TARGET_CLIENT, "Initializing database client");

        let mut manager_config = ManagerConfig::default();
        manager_config.custom_setup = Box::new(custom_hooks::setup_callback);
        let manager =
            AsyncDieselConnectionManager::new_with_config(&config.database_url, manager_config);

        let pool = Pool::builder(manager)
            .max_size(config.pool.max_size)
            .wait_timeout(Some(config.pool.connection_timeout))
            .create_timeout(Some(config.pool.connection_timeout))
            .recycle_timeout(config.pool.idle_timeout)
            .runtime(deadpool::Runtime::Tokio1)
            .post_create(Hook::sync_fn(custom_hooks::post_create))
            .pre_recycle(Hook::sync_fn(custom_hooks::pre_recycle))
            .post_recycle(Hook::sync_fn(custom_hooks::post_recycle))
            .build()
            .map_err(|e| {
                error!(target: TRACING_TARGET_CLIENT, error = %e, "Failed to create connection pool");
                PgError::Unexpected(format!("Failed to build connection pool: {}", e).into())
            })?;

        Ok(Self {
            inner: Arc::new(PgClientInner { pool, config }),
        })
    }

    /// Creates a new database client with the provided configuration.
    ///
    /// This will establish a connection pool and verify connectivity to the database.
    ///
    /// # Arguments
    ///
    /// * `config` - Database configuration including connection details and pool settings
    ///
    /// # Errors
    ///
    /// Returns an error if:
    ///
    /// - The database connection cannot be established
    /// - Pool configuration is invalid
    /// - Database connectivity test fails
    #[instrument(skip(config), target = TRACING_TARGET_CLIENT, fields(database_url = %config.database_url_masked()))]
    pub async fn new_with_test(config: PgConfig) -> PgResult<Self> {
        let this = Self::new(config)?;

        // Test connectivity
        debug!(target: TRACING_TARGET_CONNECTION, "Testing database connectivity");
        let mut conn: PooledConnection = this.inner.pool.get().await.map_err(
            |e: deadpool::managed::PoolError<diesel_async::pooled_connection::PoolError>| {
                error!(
                    target: TRACING_TARGET_CONNECTION, error = %e,
                    "Failed to get connection from pool during initialization"
                );
                PgError::from(e)
            },
        )?;

        // Perform a simple connectivity test
        #[derive(diesel::QueryableByName)]
        struct ConnectivityTest {
            #[diesel(sql_type = diesel::sql_types::Integer)]
            #[allow(dead_code)]
            result: i32,
        }

        let _: ConnectivityTest = diesel::sql_query("SELECT 1 as result")
            .get_result(&mut *conn)
            .await
            .map_err(|e| {
                error!(target: TRACING_TARGET_CONNECTION, error = %e, "Database connectivity test failed");
                PgError::from(e)
            })?;

        info!(
            target: TRACING_TARGET_CLIENT,
            max_size = this.inner.config.pool.max_size,
            connection_timeout = ?this.inner.config.pool.connection_timeout,
            idle_timeout = ?this.inner.config.pool.idle_timeout,
            "Database client initialized successfully"
        );

        Ok(this)
    }

    /// Gets a connection from the pool.
    ///
    /// This method will wait up to the configured timeout for an available connection.
    ///
    /// # Errors
    ///
    /// Returns an error if no connection is available within the timeout period.
    #[instrument(skip(self), target = TRACING_TARGET_CONNECTION)]
    pub async fn get_connection(&self) -> PgResult<PooledConnection> {
        debug!(target: TRACING_TARGET_CONNECTION, "Acquiring connection from pool");

        let start = std::time::Instant::now();
        let conn = self.inner.pool.get().await.map_err(|e| {
            error!(
                target: TRACING_TARGET_CONNECTION,
                error = %e,
                elapsed = ?start.elapsed(),
                "Failed to acquire connection from pool"
            );
            PgError::from(e)
        })?;

        let elapsed = start.elapsed();
        if elapsed > Duration::from_millis(100) {
            warn!(
                target: TRACING_TARGET_CONNECTION,
                elapsed = ?elapsed,
                "Connection acquisition took longer than expected"
            );
        }

        debug!(target: TRACING_TARGET_CONNECTION, elapsed = ?elapsed, "Connection acquired successfully");
        Ok(conn)
    }

    /// Gets the current pool status and statistics.
    ///
    /// This method provides insights into the connection pool state for monitoring
    /// and debugging purposes.
    #[inline]
    pub fn pool_status(&self) -> PgPoolStatus {
        let status = self.inner.pool.status();
        PgPoolStatus {
            max_size: status.max_size,
            size: status.size,
            available: status.available,
            waiting: status.waiting,
        }
    }

    /// Gets the database configuration used by this client.
    #[inline]
    pub fn config(&self) -> &PgConfig {
        &self.inner.config
    }
}

impl std::fmt::Debug for PgClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let pool_status = self.pool_status();
        f.debug_struct("PgDatabase")
            .field("database_url", &self.inner.config.database_url_masked())
            .field("pool_max_size", &self.inner.config.pool.max_size)
            .field("pool_current_size", &pool_status.size)
            .field("pool_available", &pool_status.available)
            .field("pool_waiting", &pool_status.waiting)
            .field(
                "connection_timeout",
                &self.inner.config.pool.connection_timeout,
            )
            .field("idle_timeout", &self.inner.config.pool.idle_timeout)
            .finish()
    }
}
