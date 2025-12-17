use std::path::PathBuf;

use derive_builder::Builder;
use nvisy_nats::{NatsClient, NatsConfig};
use nvisy_postgres::{PgClient, PgClientMigrationExt, PgConfig};
use serde::{Deserialize, Serialize};

use crate::service::{AuthKeysConfig, Result, Error, SessionKeys};

/// Default values for configuration options.
mod defaults {
    use std::path::PathBuf;

    /// Default Postgres connection string for development.
    pub const POSTGRES_ENDPOINT: &str = "postgresql://postgres:postgres@localhost:5432/postgres";

    /// Default NATS URL.
    pub const NATS_URL: &str = "nats://127.0.0.1:4222";

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
#[derive(Debug, Clone, Serialize, Deserialize, Builder)]
#[must_use = "config does nothing unless you use it"]
#[builder(
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(validate = "Self::validate")
)]
pub struct ServiceConfig {
    /// Postgres database configuration.
    #[builder(default = "PgConfig::new(defaults::POSTGRES_ENDPOINT)")]
    pub postgres_config: PgConfig,

    /// NATS configuration.
    #[builder(default = "NatsConfig::new(defaults::NATS_URL)")]
    pub nats_config: NatsConfig,

    /// File path to the JWT decoding (public) key used for sessions.
    #[builder(default = "defaults::auth_decoding_key()")]
    pub auth_decoding_key: PathBuf,

    /// File path to the JWT encode (private) key used for sessions.
    #[builder(default = "defaults::auth_encoding_key()")]
    pub auth_encoding_key: PathBuf,
}

impl ServiceConfig {
    /// Creates a new configuration builder.
    pub fn builder() -> ServiceConfigBuilder {
        ServiceConfigBuilder::default()
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
    pub async fn load_auth_keys(&self) -> Result<SessionKeys> {
        let config = AuthKeysConfig::new(&self.auth_decoding_key, &self.auth_encoding_key);
        SessionKeys::from_config(config).await
    }
}

impl ServiceConfigBuilder {
    /// Wrapper for builder validation that returns String errors.
    fn validate(_builder: &ServiceConfigBuilder) -> Result<(), String> {
        // NATS config validation is handled by NatsConfig::validate()
        // Postgres config validation is handled by PgConfig::validate()
        Ok(())
    }
}

#[cfg(debug_assertions)]
impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            postgres_config: PgConfig::new(defaults::POSTGRES_ENDPOINT),
            nats_config: NatsConfig::new(defaults::NATS_URL),
            auth_decoding_key: defaults::auth_decoding_key(),
            auth_encoding_key: defaults::auth_encoding_key(),
        }
    }
}
