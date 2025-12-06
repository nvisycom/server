//! Advanced database connection pool configuration.
//!
//! The module provides comprehensive configuration options for PostgreSQL connection pools,
//! with built-in validation, sensible defaults, and optimization presets for different
//! deployment scenarios.

use std::fmt;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tracing::{debug, instrument, warn};

use crate::{PgClient, PgError, PgResult, TRACING_TARGET_CLIENT};

/// Complete database configuration including connection string and pool settings.
///
/// This configuration system provides type-safe, validated settings for PostgreSQL
/// connections and connection pools with optimization presets for different
/// deployment scenarios.
///
/// ## Example
///
/// ```rust,no_run
/// use nvisy_postgres::client::{PgConfig, PgPoolConfig};
/// use std::time::Duration;
///
/// let config = PgConfig::from_env()?;
/// // or
/// let config = PgConfig::new(
///     "postgresql://user:pass@localhost/db".to_string(),
///     PgPoolConfig::default()
/// );
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug)]
#[must_use = "database configurations must be used to create connection pools"]
pub struct PgConfig {
    /// PostgreSQL connection URL
    pub database_url: String,
    /// Connection pool configuration
    pub pool: PgPoolConfig,
}

/// Connection pool configuration with comprehensive timeout and sizing options.
///
/// ## Connection Management
///
/// - `max_size`: Maximum number of connections in the pool
/// - `connection_timeout`: Timeout for connection operations
/// - `idle_timeout`: How long to keep idle connections
#[derive(Debug, Clone, Serialize, Deserialize)]
#[must_use = "pool configurations must be used to create connection pools"]
pub struct PgPoolConfig {
    /// Maximum number of connections in the pool.
    ///
    /// Default: 10
    /// Range: 1-100 connections
    pub max_size: u32,

    /// Timeout for connection operations (create, acquire, recycle).
    ///
    /// This affects how long various connection operations will wait.
    ///
    /// Default: 30 seconds
    /// Range: 1 second - 300 seconds
    pub connection_timeout: Duration,

    /// How long to keep idle connections in the pool.
    ///
    /// Connections idle longer than this will be closed and removed.
    /// Use `None` to keep connections indefinitely.
    ///
    /// Default: Some(10 minutes)
    /// Range: 30 seconds - 1 hour (if specified)
    pub idle_timeout: Option<Duration>,
}

// Configuration constants
const MIN_CONNECTIONS: u32 = 1;
const MAX_CONNECTIONS: u32 = 16;
const MIN_CONN_TIMEOUT: Duration = Duration::from_millis(100);
const MAX_CONN_TIMEOUT: Duration = Duration::from_secs(300);

const MIN_IDLE_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_IDLE_TIMEOUT: Duration = Duration::from_secs(3600); // 1 hour

impl PgConfig {
    /// Creates a new database configuration.
    ///
    /// # Arguments
    ///
    /// * `database_url` - PostgreSQL connection string
    /// * `pool` - Connection pool configuration
    #[instrument(skip(database_url), fields(database_url = %Self::mask_url(&database_url)), target = TRACING_TARGET_CLIENT)]
    pub fn new(database_url: String, pool: PgPoolConfig) -> Self {
        let this = Self { database_url, pool };
        debug!(
            target: TRACING_TARGET_CLIENT,
            max_size = this.pool.max_size,
            connection_timeout = ?this.pool.connection_timeout,
            idle_timeout = ?this.pool.idle_timeout,
            "Created database configuration"
        );

        this
    }

    /// Returns a masked version of the database URL for safe logging.
    ///
    /// This removes sensitive information like passwords from the URL.
    #[inline]
    pub fn database_url_masked(&self) -> String {
        Self::mask_url(&self.database_url)
    }

    /// Masks sensitive information in a database URL.
    #[inline]
    fn mask_url(url: &str) -> String {
        // Simple password masking without url crate dependency
        if let Some(at_pos) = url.find('@') {
            if let Some(colon_pos) = url[..at_pos].rfind(':') {
                let mut masked = url.to_string();
                masked.replace_range(colon_pos + 1..at_pos, "***");
                masked
            } else {
                url.to_string()
            }
        } else {
            url.to_string()
        }
    }

    /// Sets the database URL.
    #[instrument(skip(self), target = TRACING_TARGET_CLIENT)]
    pub fn with_database_url(mut self, database_url: &str) -> Self {
        debug!(target: TRACING_TARGET_CLIENT, "Setting database URL");
        self.database_url = database_url.to_string();
        self
    }

    /// Sets the maximum number of connections in the pool.
    #[instrument(skip(self), target = TRACING_TARGET_CLIENT)]
    pub fn with_max_size(mut self, max_size: u32) -> Self {
        debug!(target: TRACING_TARGET_CLIENT, max_size, "Setting pool max size");
        self.pool.max_size = max_size;
        self
    }

    /// Sets the connection timeout.
    #[instrument(skip(self), target = TRACING_TARGET_CLIENT)]
    pub fn with_connection_timeout(mut self, timeout: Duration) -> Self {
        debug!(target: TRACING_TARGET_CLIENT, ?timeout, "Setting connection timeout");
        self.pool.connection_timeout = timeout;
        self
    }

    /// Sets the idle timeout for connections.
    #[instrument(skip(self), target = TRACING_TARGET_CLIENT)]
    pub fn with_idle_timeout(mut self, timeout: Duration) -> Self {
        debug!(target: TRACING_TARGET_CLIENT, ?timeout, "Setting idle timeout");
        self.pool.idle_timeout = Some(timeout);
        self
    }

    /// Builds a new database instance with the given configuration.
    ///
    /// Validates the configuration for consistency and safety.
    #[instrument(skip(self), target = TRACING_TARGET_CLIENT)]
    pub fn build(self) -> PgResult<PgClient> {
        debug!(target: TRACING_TARGET_CLIENT, "Validating database configuration");

        // Validate database URL
        if self.database_url.is_empty() {
            return Err(PgError::Config("database_url cannot be empty".to_string()));
        }

        // Basic URL validation
        if !self.database_url.starts_with("postgres://")
            && !self.database_url.starts_with("postgresql://")
        {
            warn!(target: TRACING_TARGET_CLIENT, "Database URL may not be a PostgreSQL URL");
        }

        self.pool.validate()?;

        debug!(target: TRACING_TARGET_CLIENT, "Database configuration validation passed");
        PgClient::new(self)
    }
}

impl PgPoolConfig {
    /// Creates a new pool configuration with default values.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an optimized configuration for high-load single server deployments.
    #[inline]
    pub fn single_server() -> Self {
        Self {
            max_size: 20,
            connection_timeout: Duration::from_secs(30),
            idle_timeout: Some(Duration::from_secs(600)), // 10 minutes
        }
    }

    /// Creates an optimized configuration for multi-server deployments.
    #[inline]
    pub fn multi_server() -> Self {
        Self {
            max_size: 10,
            connection_timeout: Duration::from_secs(30),
            idle_timeout: Some(Duration::from_secs(300)), // 5 minutes
        }
    }

    /// Validates the pool configuration.
    #[instrument(skip(self), target = TRACING_TARGET_CLIENT)]
    pub fn validate(&self) -> PgResult<()> {
        // Validate connection count
        if !(MIN_CONNECTIONS..=MAX_CONNECTIONS).contains(&self.max_size) {
            return Err(PgError::Config(format!(
                "max_size must be between {} and {}",
                MIN_CONNECTIONS, MAX_CONNECTIONS
            )));
        }

        // Validate connection timeout
        if self.connection_timeout < MIN_CONN_TIMEOUT || self.connection_timeout > MAX_CONN_TIMEOUT
        {
            return Err(PgError::Config(format!(
                "connection_timeout must be between {:?} and {:?}",
                MIN_CONN_TIMEOUT, MAX_CONN_TIMEOUT
            )));
        }

        // Validate idle timeout if present
        if let Some(timeout) = self.idle_timeout
            && (timeout < MIN_IDLE_TIMEOUT || timeout > MAX_IDLE_TIMEOUT)
        {
            return Err(PgError::Config(format!(
                "idle_timeout must be between {:?} and {:?}",
                MIN_IDLE_TIMEOUT, MAX_IDLE_TIMEOUT
            )));
        }

        Ok(())
    }
}

impl Default for PgPoolConfig {
    /// Creates a default pool configuration suitable for most applications.
    ///
    /// This uses conservative settings that work well in most deployment scenarios.
    fn default() -> Self {
        Self {
            max_size: 10,
            connection_timeout: Duration::from_secs(30),
            idle_timeout: Some(Duration::from_secs(600)), // 10 minutes
        }
    }
}

impl fmt::Display for PgConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "DatabaseConfig(url: {}, max_size: {}, connection_timeout: {:?}, idle_timeout: {:?})",
            self.database_url_masked(),
            self.pool.max_size,
            self.pool.connection_timeout,
            self.pool.idle_timeout
        )
    }
}
