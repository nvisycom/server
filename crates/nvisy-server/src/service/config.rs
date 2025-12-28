use std::path::PathBuf;

#[cfg(all(not(test), feature = "config"))]
use clap::Args;
#[cfg(test)]
use clap::Parser;
use nvisy_nats::{NatsClient, NatsConfig};
use nvisy_postgres::{PgClient, PgClientMigrationExt, PgConfig};
use serde::{Deserialize, Serialize};

use crate::service::{AuthKeysConfig, Error, Result, SessionKeys};

/// Default values for configuration options.
mod defaults {
    use std::path::PathBuf;

    /// Default path to JWT decoding key.
    pub fn auth_decoding_key() -> PathBuf {
        "./public.pem".into()
    }

    /// Default path to JWT encoding key.
    pub fn auth_encoding_key() -> PathBuf {
        "./private.pem".into()
    }
}

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

    /// File path to the JWT decoding (public) key used for sessions.
    #[cfg_attr(
        feature = "config",
        arg(long, env = "AUTH_PUBLIC_PEM_FILEPATH", default_value = "./public.pem")
    )]
    #[serde(default = "defaults::auth_decoding_key")]
    pub auth_decoding_key: PathBuf,

    /// File path to the JWT encode (private) key used for sessions.
    #[cfg_attr(
        feature = "config",
        arg(
            long,
            env = "AUTH_PRIVATE_PEM_FILEPATH",
            default_value = "./private.pem"
        )
    )]
    #[serde(default = "defaults::auth_encoding_key")]
    pub auth_encoding_key: PathBuf,
}

impl ServiceConfig {
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
    pub async fn load_auth_keys(&self) -> Result<SessionKeys> {
        let config = AuthKeysConfig::new(&self.auth_decoding_key, &self.auth_encoding_key);
        SessionKeys::from_config(config).await
    }
}

#[cfg(test)]
impl Default for ServiceConfig {
    fn default() -> Self {
        Self::parse()
    }
}
