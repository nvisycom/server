use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use deadpool::managed::{Hook, Pool};
use derive_more::{Deref, DerefMut};
use diesel_async::pooled_connection::{AsyncDieselConnectionManager, ManagerConfig};
use diesel_async::scoped_futures::ScopedBoxFuture;
use diesel_async::{AsyncConnection, RunQueryDsl};

use super::custom_hooks;
use crate::{
    ConnectionPool, PgConfig, PgError, PgResult, PooledConnection, TRACING_TARGET_CONNECTION,
};

/// Connection pool status information.
#[derive(Debug, Clone)]
pub struct PgPoolStatus {
    /// Maximum number of connections in the pool
    pub max_size: usize,
    /// Current number of connections in the pool
    pub size: usize,
    /// Number of available connections
    pub available: usize,
    /// Number of requests waiting for connections
    pub waiting: usize,
}

impl PgPoolStatus {
    /// Returns the utilization percentage of the pool (0.0 to 1.0).
    #[inline]
    pub fn utilization(&self) -> f64 {
        if self.max_size == 0 {
            0.0
        } else {
            (self.size - self.available) as f64 / self.max_size as f64
        }
    }

    /// Returns whether the pool is under pressure (high utilization or waiting requests).
    #[inline]
    pub fn is_under_pressure(&self) -> bool {
        self.waiting > 0 || self.utilization() > 0.8
    }
}

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
    #[tracing::instrument(
        skip(config),
        target = TRACING_TARGET_CONNECTION,
        fields(database_url = %config.database_url_masked())
    )]
    pub fn new(config: PgConfig) -> PgResult<Self> {
        tracing::info!(target: TRACING_TARGET_CONNECTION, "Initializing database client");

        let mut manager_config = ManagerConfig::default();
        manager_config.custom_setup = Box::new(custom_hooks::setup_callback);
        let manager =
            AsyncDieselConnectionManager::new_with_config(&config.postgres_url, manager_config);

        let pool = Pool::builder(manager)
            .max_size(config.postgres_max_connections as usize)
            .wait_timeout(config.connection_timeout())
            .create_timeout(config.connection_timeout())
            .recycle_timeout(config.idle_timeout())
            .runtime(deadpool::Runtime::Tokio1)
            .post_create(Hook::sync_fn(custom_hooks::post_create))
            .pre_recycle(Hook::sync_fn(custom_hooks::pre_recycle))
            .post_recycle(Hook::sync_fn(custom_hooks::post_recycle))
            .build()
            .map_err(|e| {
                tracing::error!(target: TRACING_TARGET_CONNECTION, error = %e, "Failed to create connection pool");
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
    #[tracing::instrument(
        skip(config),
        target = TRACING_TARGET_CONNECTION,
        fields(database_url = %config.database_url_masked())
    )]
    pub async fn new_with_test(config: PgConfig) -> PgResult<Self> {
        let this = Self::new(config)?;

        // Test connectivity
        tracing::debug!(target: TRACING_TARGET_CONNECTION, "Testing database connectivity");
        let mut conn: PooledConnection = this.inner.pool.get().await.map_err(
            |e: deadpool::managed::PoolError<diesel_async::pooled_connection::PoolError>| {
                tracing::error!(
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
                tracing::error!(target: TRACING_TARGET_CONNECTION, error = %e, "Database connectivity test failed");
                PgError::from(e)
            })?;

        tracing::info!(
            target: TRACING_TARGET_CONNECTION,
            max_connections = this.inner.config.postgres_max_connections,
            connection_timeout_secs = this.inner.config.postgres_connection_timeout_secs,
            idle_timeout_secs = this.inner.config.postgres_idle_timeout_secs,
            "Database client initialized successfully"
        );

        Ok(this)
    }

    /// Gets a connection from the pool.
    ///
    /// Returns a [`PgConn`] wrapper that implements all repository traits.
    /// This method will wait up to the configured timeout for an available connection.
    ///
    /// # Errors
    ///
    /// Returns an error if no connection is available within the timeout period.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_CONNECTION)]
    pub async fn get_connection(&self) -> PgResult<PgConn> {
        tracing::debug!(target: TRACING_TARGET_CONNECTION, "Acquiring connection from pool");

        let start = std::time::Instant::now();
        let conn = self.inner.pool.get().await.map_err(|e| {
            tracing::error!(
                target: TRACING_TARGET_CONNECTION,
                error = %e,
                elapsed = ?start.elapsed(),
                "Failed to acquire connection from pool"
            );
            PgError::from(e)
        })?;

        let elapsed = start.elapsed();
        if elapsed > Duration::from_millis(100) {
            tracing::warn!(
                target: TRACING_TARGET_CONNECTION,
                elapsed = ?elapsed,
                "Connection acquisition took longer than expected"
            );
        }

        tracing::debug!(target: TRACING_TARGET_CONNECTION, elapsed = ?elapsed, "Connection acquired successfully");
        Ok(PgConn::new(conn))
    }

    /// Gets a raw pooled connection from the pool.
    ///
    /// This is intended for internal use by the migration module.
    pub(crate) async fn get_pooled_connection(&self) -> PgResult<PooledConnection> {
        let conn = self.inner.pool.get().await.map_err(PgError::from)?;
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
            .field(
                "pool_max_connections",
                &self.inner.config.postgres_max_connections,
            )
            .field("pool_current_size", &pool_status.size)
            .field("pool_available", &pool_status.available)
            .field("pool_waiting", &pool_status.waiting)
            .field(
                "connection_timeout_secs",
                &self.inner.config.postgres_connection_timeout_secs,
            )
            .field(
                "idle_timeout_secs",
                &self.inner.config.postgres_idle_timeout_secs,
            )
            .finish()
    }
}

/// A wrapper around a pooled database connection.
///
/// `PgConn` owns a connection obtained from the connection pool and implements
/// all repository traits (e.g., [`AccountRepository`], [`ProjectRepository`])
/// via [`Deref`] to the underlying [`AsyncPgConnection`].
/// When dropped, the connection is automatically returned to the pool.
///
/// # Usage
///
/// Obtain a `PgConn` from [`PgClient::get_connection`] and use it to execute
/// database operations through the repository traits.
///
/// ```ignore
/// let mut conn = pg_client.get_connection().await?;
/// let account = conn.find_account_by_id(account_id).await?;
/// ```
///
/// [`AccountRepository`]: crate::query::AccountRepository
/// [`ProjectRepository`]: crate::query::ProjectRepository
/// [`PgClient::get_connection`]: crate::PgClient::get_connection
/// [`AsyncPgConnection`]: crate::PgConnection
#[derive(Deref, DerefMut)]
pub struct PgConn {
    #[deref]
    #[deref_mut]
    conn: PooledConnection,
}

impl PgConn {
    /// Creates a new connection wrapper from a pooled connection.
    pub fn new(conn: PooledConnection) -> Self {
        Self { conn }
    }

    /// Executes the given function within a database transaction.
    ///
    /// If the function returns `Ok`, the transaction is committed.
    /// If the function returns `Err`, the transaction is rolled back.
    ///
    /// # Example
    ///
    /// ```ignore
    /// conn.transaction(|conn| {
    ///     Box::pin(async move {
    ///         // Perform multiple database operations
    ///         diesel::insert_into(table).values(&data).execute(conn).await?;
    ///         diesel::update(other_table).set(&changes).execute(conn).await?;
    ///         Ok(result)
    ///     })
    /// }).await?;
    /// ```
    pub async fn transaction<'a, T, E, F>(&mut self, f: F) -> Result<T, E>
    where
        F: for<'r> FnOnce(&'r mut PooledConnection) -> ScopedBoxFuture<'a, 'r, Result<T, E>>
            + Send
            + 'a,
        T: Send + 'a,
        E: From<diesel::result::Error> + Send + 'a,
    {
        self.conn.transaction(f).await
    }
}

impl fmt::Debug for PgConn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PgConn").finish_non_exhaustive()
    }
}
