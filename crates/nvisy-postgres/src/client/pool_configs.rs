//! Advanced database connection pool configuration.
//!
//! The module provides comprehensive configuration options for PostgreSQL connection pools,
//! with built-in validation, sensible defaults, and optimization presets for different
//! deployment scenarios.

use std::fmt;
use std::num::NonZeroU32;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{PgClient, PgError, PgResult, TRACING_TARGET_CONNECTION};

/// Complete database configuration including connection string and pool settings.
///
/// This configuration system provides type-safe, validated settings for PostgreSQL
/// connections and connection pools with optimization presets for different
/// deployment scenarios.
///
/// ## Example
///
/// ```rust,no_run
/// use nvisy_postgres::PgConfig;
///
/// let config = PgConfig::new("postgresql://user:pass@localhost/db");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
#[must_use = "database configurations must be used to create connection pools"]
pub struct PgConfig {
    /// PostgreSQL connection URL
    pub database_url: String,

    /// Maximum number of connections in the pool.
    ///
    /// Default: 10
    /// Range: 1-100 connections
    pub max_size: Option<NonZeroU32>,

    /// Timeout for connection operations (create, acquire, recycle).
    ///
    /// This affects how long various connection operations will wait.
    ///
    /// Default: 30 seconds
    /// Range: 1 second - 300 seconds
    pub connection_timeout: Option<Duration>,

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
const MIN_CONNECTIONS: u32 = 2;
const MAX_CONNECTIONS: u32 = 16;

const MIN_CONN_TIMEOUT: Duration = Duration::from_millis(100);
const MAX_CONN_TIMEOUT: Duration = Duration::from_secs(300);

const MIN_IDLE_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_IDLE_TIMEOUT: Duration = Duration::from_secs(3600);

impl PgConfig {
    /// Creates a new database configuration with default pool settings.
    ///
    /// # Arguments
    ///
    /// * `database_url` - PostgreSQL connection string
    #[tracing::instrument(
        skip(database_url),
        target = TRACING_TARGET_CONNECTION
    )]
    pub fn new(database_url: impl Into<String>) -> Self {
        let this = Self {
            database_url: database_url.into(),
            max_size: None,
            connection_timeout: None,
            idle_timeout: None,
        };

        tracing::debug!(
            target: TRACING_TARGET_CONNECTION,
            database_url = %this.database_url_masked(),
            max_size = this.max_size,
            connection_timeout = ?this.connection_timeout,
            idle_timeout = ?this.idle_timeout,
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

    /// Returns the database URL.
    #[inline]
    pub fn get_database_url(&self) -> &str {
        &self.database_url
    }

    /// Returns the maximum pool size, or the default if not set.
    #[inline]
    pub fn get_max_size(&self) -> usize {
        self.max_size.map(|n| n.get() as usize).unwrap_or(10)
    }

    /// Returns the connection timeout.
    #[inline]
    pub fn get_connection_timeout(&self) -> Option<Duration> {
        self.connection_timeout
    }

    /// Returns the idle timeout.
    #[inline]
    pub fn get_idle_timeout(&self) -> Option<Duration> {
        self.idle_timeout
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
    #[tracing::instrument(skip(self), target = TRACING_TARGET_CONNECTION)]
    pub fn with_database_url(mut self, database_url: &str) -> Self {
        tracing::debug!(target: TRACING_TARGET_CONNECTION, "Setting database URL");
        self.database_url = database_url.to_string();
        self
    }

    /// Sets the maximum number of connections in the pool.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_CONNECTION)]
    pub fn with_max_size(mut self, max_size: u32) -> Self {
        tracing::debug!(target: TRACING_TARGET_CONNECTION, max_size, "Setting pool max size");
        self.max_size = NonZeroU32::new(max_size);
        self
    }

    /// Sets the connection timeout.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_CONNECTION)]
    pub fn with_connection_timeout(mut self, timeout: Duration) -> Self {
        tracing::debug!(target: TRACING_TARGET_CONNECTION, ?timeout, "Setting connection timeout");
        self.connection_timeout = Some(timeout);
        self
    }

    /// Sets the idle timeout for connections.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_CONNECTION)]
    pub fn with_idle_timeout(mut self, timeout: Duration) -> Self {
        tracing::debug!(target: TRACING_TARGET_CONNECTION, ?timeout, "Setting idle timeout");
        self.idle_timeout = Some(timeout);
        self
    }

    /// Creates an optimized configuration for high-load single server deployments.
    pub fn single_server(database_url: impl Into<String>) -> Self {
        Self {
            database_url: database_url.into(),
            max_size: NonZeroU32::new(20),
            connection_timeout: Some(Duration::from_secs(30)),
            idle_timeout: Some(Duration::from_secs(600)),
        }
    }

    /// Creates an optimized configuration for multi-server deployments.
    pub fn multi_server(database_url: impl Into<String>) -> Self {
        Self {
            database_url: database_url.into(),
            max_size: NonZeroU32::new(10),
            connection_timeout: Some(Duration::from_secs(30)),
            idle_timeout: Some(Duration::from_secs(300)),
        }
    }

    /// Builds a new database instance with the given configuration.
    ///
    /// Validates the configuration for consistency and safety.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_CONNECTION)]
    pub fn build(self) -> PgResult<PgClient> {
        tracing::debug!(target: TRACING_TARGET_CONNECTION, "Validating database configuration");

        // Validate database URL
        if self.database_url.is_empty() {
            return Err(PgError::Config("database_url cannot be empty".to_string()));
        }

        // Basic URL validation
        if !self.database_url.starts_with("postgres://")
            && !self.database_url.starts_with("postgresql://")
        {
            tracing::warn!(target: TRACING_TARGET_CONNECTION, "Database URL may not be a PostgreSQL URL");
        }

        // Validate connection count
        if let Some(max_size) = self.max_size {
            let max_size = max_size.get();
            if !(MIN_CONNECTIONS..=MAX_CONNECTIONS).contains(&max_size) {
                return Err(PgError::Config(format!(
                    "max_size must be between {} and {}",
                    MIN_CONNECTIONS, MAX_CONNECTIONS
                )));
            }
        }

        // Validate connection timeout
        if let Some(connection_timeout) = self.connection_timeout
            && (connection_timeout < MIN_CONN_TIMEOUT || connection_timeout > MAX_CONN_TIMEOUT)
        {
            return Err(PgError::Config(format!(
                "connection_timeout must be between {:?} and {:?}",
                MIN_CONN_TIMEOUT, MAX_CONN_TIMEOUT
            )));
        }

        // Validate idle timeout if present
        if let Some(idle_timeout) = self.idle_timeout
            && (idle_timeout < MIN_IDLE_TIMEOUT || idle_timeout > MAX_IDLE_TIMEOUT)
        {
            return Err(PgError::Config(format!(
                "idle_timeout must be between {:?} and {:?}",
                MIN_IDLE_TIMEOUT, MAX_IDLE_TIMEOUT
            )));
        }

        tracing::debug!(target: TRACING_TARGET_CONNECTION, "Database configuration validation passed");
        PgClient::new(self)
    }
}

impl Default for PgConfig {
    /// Creates a default configuration with a development database URL.
    ///
    /// This uses conservative settings that work well in most deployment scenarios.
    /// Default database URL: `postgresql://postgres:postgres@localhost:5432/postgres`
    ///
    /// For production use, always override the database_url with your actual connection string.
    #[cfg(debug_assertions)]
    fn default() -> Self {
        Self::single_server("postgresql://postgres:postgres@localhost:5432/postgres")
    }
}

impl fmt::Display for PgConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "DatabaseConfig(url: {}, max_size: {:?}, connection_timeout: {:?}, idle_timeout: {:?})",
            self.database_url_masked(),
            self.max_size,
            self.connection_timeout,
            self.idle_timeout
        )
    }
}
