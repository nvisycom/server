//! Advanced database connection pool configuration.
//!
//! The module provides comprehensive configuration options for PostgreSQL connection pools,
//! with built-in validation, sensible defaults, and optimization presets for different
//! deployment scenarios.

use std::fmt;
use std::time::Duration;

#[cfg(feature = "config")]
use clap::Args;
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
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "config", derive(Args))]
#[must_use = "database configurations must be used to create connection pools"]
pub struct PgConfig {
    /// PostgreSQL connection URL
    #[cfg_attr(feature = "config", arg(long = "postgres-url", env = "POSTGRES_URL"))]
    pub postgres_url: String,

    /// Maximum number of connections in the pool (2-16)
    #[cfg_attr(
        feature = "config",
        arg(
            long = "postgres-max-connections",
            env = "POSTGRES_MAX_CONNECTIONS",
            default_value = "10"
        )
    )]
    pub postgres_max_connections: u32,

    /// Connection timeout in seconds (optional)
    #[cfg_attr(
        feature = "config",
        arg(
            long = "postgres-connection-timeout-secs",
            env = "POSTGRES_CONNECTION_TIMEOUT_SECS"
        )
    )]
    pub postgres_connection_timeout_secs: Option<u64>,

    /// Idle connection timeout in seconds (optional)
    #[cfg_attr(
        feature = "config",
        arg(
            long = "postgres-idle-timeout-secs",
            env = "POSTGRES_IDLE_TIMEOUT_SECS"
        )
    )]
    pub postgres_idle_timeout_secs: Option<u64>,
}

// Configuration constants
const MIN_CONNECTIONS: u32 = 2;
const MAX_CONNECTIONS: u32 = 16;

const MIN_CONN_TIMEOUT_SECS: u64 = 1;
const MAX_CONN_TIMEOUT_SECS: u64 = 300;

const MIN_IDLE_TIMEOUT_SECS: u64 = 30;
const MAX_IDLE_TIMEOUT_SECS: u64 = 3600;

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
            postgres_url: database_url.into(),
            postgres_max_connections: 10,
            postgres_connection_timeout_secs: None,
            postgres_idle_timeout_secs: None,
        };

        tracing::debug!(
            target: TRACING_TARGET_CONNECTION,
            database_url = %this.database_url_masked(),
            max_connections = this.postgres_max_connections,
            connection_timeout_secs = ?this.postgres_connection_timeout_secs,
            idle_timeout_secs = ?this.postgres_idle_timeout_secs,
            "Created database configuration"
        );

        this
    }

    /// Returns the connection timeout as a Duration.
    #[inline]
    pub fn connection_timeout(&self) -> Option<Duration> {
        self.postgres_connection_timeout_secs
            .map(Duration::from_secs)
    }

    /// Returns the idle timeout as a Duration.
    #[inline]
    pub fn idle_timeout(&self) -> Option<Duration> {
        self.postgres_idle_timeout_secs.map(Duration::from_secs)
    }

    /// Returns a masked version of the database URL for safe logging.
    ///
    /// This removes sensitive information like passwords from the URL.
    #[inline]
    pub fn database_url_masked(&self) -> String {
        Self::mask_url(&self.postgres_url)
    }

    /// Returns the database URL.
    #[inline]
    pub fn database_url(&self) -> &str {
        &self.postgres_url
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
    #[tracing::instrument(skip(self, database_url), target = TRACING_TARGET_CONNECTION)]
    pub fn with_database_url(mut self, database_url: impl Into<String>) -> Self {
        tracing::debug!(target: TRACING_TARGET_CONNECTION, "Setting database URL");
        self.postgres_url = database_url.into();
        self
    }

    /// Sets the maximum number of connections in the pool.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_CONNECTION)]
    pub fn with_max_connections(mut self, max_connections: u32) -> Self {
        tracing::debug!(target: TRACING_TARGET_CONNECTION, max_connections, "Setting pool max connections");
        self.postgres_max_connections = max_connections;
        self
    }

    /// Sets the connection timeout in seconds.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_CONNECTION)]
    pub fn with_connection_timeout_secs(mut self, secs: u64) -> Self {
        tracing::debug!(target: TRACING_TARGET_CONNECTION, secs, "Setting connection timeout");
        self.postgres_connection_timeout_secs = Some(secs);
        self
    }

    /// Sets the idle timeout in seconds.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_CONNECTION)]
    pub fn with_idle_timeout_secs(mut self, secs: u64) -> Self {
        tracing::debug!(target: TRACING_TARGET_CONNECTION, secs, "Setting idle timeout");
        self.postgres_idle_timeout_secs = Some(secs);
        self
    }

    /// Creates an optimized configuration for high-load single server deployments.
    pub fn single_server(database_url: impl Into<String>) -> Self {
        Self {
            postgres_url: database_url.into(),
            postgres_max_connections: 20,
            postgres_connection_timeout_secs: Some(30),
            postgres_idle_timeout_secs: Some(600),
        }
    }

    /// Creates an optimized configuration for multi-server deployments.
    pub fn multi_server(database_url: impl Into<String>) -> Self {
        Self {
            postgres_url: database_url.into(),
            postgres_max_connections: 10,
            postgres_connection_timeout_secs: Some(30),
            postgres_idle_timeout_secs: Some(300),
        }
    }

    /// Validates the configuration.
    pub fn validate(&self) -> PgResult<()> {
        // Validate database URL
        if self.postgres_url.is_empty() {
            return Err(PgError::Config("database_url cannot be empty".to_string()));
        }

        // Basic URL validation
        if !self.postgres_url.starts_with("postgres://")
            && !self.postgres_url.starts_with("postgresql://")
        {
            tracing::warn!(target: TRACING_TARGET_CONNECTION, "Database URL may not be a PostgreSQL URL");
        }

        // Validate connection count
        if !(MIN_CONNECTIONS..=MAX_CONNECTIONS).contains(&self.postgres_max_connections) {
            return Err(PgError::Config(format!(
                "max_connections must be between {} and {}",
                MIN_CONNECTIONS, MAX_CONNECTIONS
            )));
        }

        // Validate connection timeout if set
        if let Some(timeout) = self.postgres_connection_timeout_secs
            && !(MIN_CONN_TIMEOUT_SECS..=MAX_CONN_TIMEOUT_SECS).contains(&timeout)
        {
            return Err(PgError::Config(format!(
                "connection_timeout_secs must be between {} and {}",
                MIN_CONN_TIMEOUT_SECS, MAX_CONN_TIMEOUT_SECS
            )));
        }

        // Validate idle timeout if set
        if let Some(timeout) = self.postgres_idle_timeout_secs
            && !(MIN_IDLE_TIMEOUT_SECS..=MAX_IDLE_TIMEOUT_SECS).contains(&timeout)
        {
            return Err(PgError::Config(format!(
                "idle_timeout_secs must be between {} and {}",
                MIN_IDLE_TIMEOUT_SECS, MAX_IDLE_TIMEOUT_SECS
            )));
        }

        Ok(())
    }

    /// Builds a new database instance with the given configuration.
    ///
    /// Validates the configuration for consistency and safety.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_CONNECTION)]
    pub fn build(self) -> PgResult<PgClient> {
        tracing::debug!(target: TRACING_TARGET_CONNECTION, "Validating database configuration");
        self.validate()?;
        tracing::debug!(target: TRACING_TARGET_CONNECTION, "Database configuration validation passed");
        PgClient::new(self)
    }
}

impl fmt::Debug for PgConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PgConfig")
            .field("postgres_url", &self.database_url_masked())
            .field("postgres_max_connections", &self.postgres_max_connections)
            .field(
                "postgres_connection_timeout_secs",
                &self.postgres_connection_timeout_secs,
            )
            .field(
                "postgres_idle_timeout_secs",
                &self.postgres_idle_timeout_secs,
            )
            .finish()
    }
}

impl fmt::Display for PgConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PgConfig(url: {}, max_connections: {}, connection_timeout: {:?}, idle_timeout: {:?})",
            self.database_url_masked(),
            self.postgres_max_connections,
            self.postgres_connection_timeout_secs,
            self.postgres_idle_timeout_secs
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_server_config() {
        let config = PgConfig::single_server("postgresql://localhost/db");
        assert!(config.postgres_url.starts_with("postgresql://"));
        assert_eq!(config.postgres_max_connections, 20);
        assert_eq!(config.connection_timeout(), Some(Duration::from_secs(30)));
        assert_eq!(config.idle_timeout(), Some(Duration::from_secs(600)));
    }

    #[test]
    fn test_new_config() {
        let config = PgConfig::new("postgresql://user:pass@localhost/db");
        assert_eq!(config.postgres_url, "postgresql://user:pass@localhost/db");
        assert_eq!(config.postgres_max_connections, 10);
        assert_eq!(config.connection_timeout(), None);
    }

    #[test]
    fn test_config_builder() {
        let config = PgConfig::new("postgresql://localhost/db")
            .with_max_connections(8)
            .with_connection_timeout_secs(60)
            .with_idle_timeout_secs(300);

        assert_eq!(config.postgres_max_connections, 8);
        assert_eq!(config.connection_timeout(), Some(Duration::from_secs(60)));
        assert_eq!(config.idle_timeout(), Some(Duration::from_secs(300)));
    }

    #[test]
    fn test_no_timeout() {
        let config = PgConfig::new("postgresql://localhost/db");
        assert_eq!(config.connection_timeout(), None);
        assert_eq!(config.idle_timeout(), None);
    }

    #[test]
    fn test_url_masking() {
        let config = PgConfig::new("postgresql://user:secret@localhost/db");
        assert_eq!(
            config.database_url_masked(),
            "postgresql://user:***@localhost/db"
        );
    }

    #[test]
    fn test_validation() {
        let valid_config = PgConfig::new("postgresql://localhost/db")
            .with_max_connections(10)
            .with_connection_timeout_secs(30);
        assert!(valid_config.validate().is_ok());

        let empty_url = PgConfig::new("");
        assert!(empty_url.validate().is_err());

        let invalid_connections =
            PgConfig::new("postgresql://localhost/db").with_max_connections(100);
        assert!(invalid_connections.validate().is_err());
    }

    #[test]
    fn test_duration_helpers() {
        let config = PgConfig::new("postgresql://localhost/db")
            .with_connection_timeout_secs(45)
            .with_idle_timeout_secs(120);

        assert_eq!(config.connection_timeout(), Some(Duration::from_secs(45)));
        assert_eq!(config.idle_timeout(), Some(Duration::from_secs(120)));
    }
}
