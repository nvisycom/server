#[cfg(all(not(test), feature = "config"))]
use clap::Args;
#[cfg(test)]
use clap::Parser;
use nvisy_nats::{NatsClient, NatsConfig};
use nvisy_postgres::{PgClient, PgClientMigrationExt, PgConfig};
use serde::{Deserialize, Serialize};

use crate::service::security::{SessionKeys, SessionKeysConfig};
use crate::service::{Error, Result};

/// App [`state`] configuration.
///
/// [`state`]: crate::service::ServiceState
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(Parser))]
#[cfg_attr(all(not(test), feature = "config"), derive(Args))]
#[must_use = "config does nothing unless you use it"]
pub struct ServiceConfig {
    /// Postgres database configuration.
    #[cfg_attr(any(test, feature = "config"), command(flatten))]
    pub postgres_config: PgConfig,

    /// NATS configuration.
    #[cfg_attr(any(test, feature = "config"), command(flatten))]
    pub nats_config: NatsConfig,

    /// Authentication key paths configuration.
    #[cfg_attr(any(test, feature = "config"), command(flatten))]
    pub session_config: SessionKeysConfig,
}

impl ServiceConfig {
    /// Creates a new ServiceConfig by parsing from environment variables.
    ///
    /// This method loads `.env` file if present and parses configuration
    /// from environment variables and command line arguments.
    #[cfg(test)]
    pub fn from_env() -> anyhow::Result<Self> {
        dotenvy::dotenv()?;
        Ok(Self::parse())
    }

    /// Connects to Postgres database and runs migrations.
    pub async fn connect_postgres(&self) -> Result<PgClient> {
        let pg_client = PgClient::new(self.postgres_config.clone()).map_err(|e| {
            Error::internal("postgres", "Failed to create database client").with_source(e)
        })?;

        pg_client.run_pending_migrations().await.map_err(|e| {
            Error::internal("postgres", "Failed to apply database migrations").with_source(e)
        })?;

        Ok(pg_client)
    }

    /// Connects to NATS server.
    pub async fn connect_nats(&self) -> Result<NatsClient> {
        NatsClient::connect(self.nats_config.clone())
            .await
            .map_err(|e| Error::external("NATS", "Failed to connect to NATS").with_source(e))
    }

    /// Loads authentication keys from configured paths.
    pub async fn load_session_keys(&self) -> Result<SessionKeys> {
        SessionKeys::from_config(&self.session_config).await
    }
}
