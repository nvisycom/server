use std::path::PathBuf;

use derive_builder::Builder;
use nvisy_nats::{NatsClient, NatsConfig};
use nvisy_portkey::{LlmClient, LlmConfig};
use nvisy_postgres::{PgClient, PgClientMigrationExt, PgConfig};
use serde::{Deserialize, Serialize};

use crate::service::{AuthKeysConfig, Result, ServiceError, SessionKeys};

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

    /// Default OpenRouter API key for development.
    pub fn openrouter_api_key() -> String {
        format!("sk-or-v1-{}", "A".repeat(64))
    }

    /// Default PostgreSQL max connections.
    pub const POSTGRES_MAX_CONNECTIONS: u32 = 10;

    /// Default PostgreSQL connection timeout in seconds.
    pub const POSTGRES_CONNECTION_TIMEOUT_SECS: u64 = 30;
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
    /// Postgres database connection string.
    #[builder(default = "defaults::POSTGRES_ENDPOINT.to_string()")]
    pub postgres_endpoint: String,

    /// Maximum number of connections in the Postgres connection pool.
    #[builder(default = "defaults::POSTGRES_MAX_CONNECTIONS")]
    pub postgres_max_connections: u32,

    /// Connection timeout for Postgres operations in seconds.
    #[builder(default = "defaults::POSTGRES_CONNECTION_TIMEOUT_SECS")]
    pub postgres_connection_timeout_secs: u64,

    /// NATS server URL.
    #[builder(default = "defaults::NATS_URL.to_string()")]
    pub nats_url: String,

    /// File path to the JWT decoding (public) key used for sessions.
    #[builder(default = "defaults::auth_decoding_key()")]
    pub auth_decoding_key: PathBuf,

    /// File path to the JWT encode (private) key used for sessions.
    #[builder(default = "defaults::auth_encoding_key()")]
    pub auth_encoding_key: PathBuf,

    /// OpenRouter API key.
    #[builder(default = "defaults::openrouter_api_key()")]
    pub openrouter_api_key: String,

    /// OpenRouter base URL.
    #[builder(default)]
    pub openrouter_base_url: Option<String>,
}

impl ServiceConfig {
    /// Creates a new configuration builder.
    pub fn builder() -> ServiceConfigBuilder {
        ServiceConfigBuilder::default()
    }

    /// Connects to Postgres database and runs migrations.
    pub async fn connect_postgres(&self) -> Result<PgClient> {
        let pool_config = nvisy_postgres::PgPoolConfig {
            max_size: self.postgres_max_connections,
            connection_timeout: std::time::Duration::from_secs(
                self.postgres_connection_timeout_secs,
            ),
            ..nvisy_postgres::PgPoolConfig::default()
        };
        let config = PgConfig::new(self.postgres_endpoint.clone(), pool_config);
        let pg_client = PgClient::new(config).map_err(|e| {
            ServiceError::internal("postgres", "Failed to create database client").with_source(e)
        })?;

        pg_client.run_pending_migrations().await.map_err(|e| {
            ServiceError::internal("postgres", "Failed to apply database migrations").with_source(e)
        })?;

        Ok(pg_client)
    }

    /// Connects to OpenRouter LLM service.
    pub async fn connect_llm(&self) -> Result<LlmClient> {
        let config = {
            let mut builder = LlmConfig::builder().with_api_key(&self.openrouter_api_key);
            if let Some(base_url) = &self.openrouter_base_url {
                builder = builder.with_base_url(base_url.clone());
            }
            builder.build()
        }
        .map_err(|e| {
            ServiceError::external("OpenRouter", "Failed to build LLM config").with_source(e)
        })?;

        LlmClient::new(config).map_err(|e| {
            ServiceError::external("OpenRouter", "Failed to create LLM client").with_source(e)
        })
    }

    /// Connects to NATS server.
    pub async fn connect_nats(&self) -> Result<NatsClient> {
        let config = NatsConfig::new(&self.nats_url);
        NatsClient::connect(config)
            .await
            .map_err(|e| ServiceError::external("NATS", "Failed to connect to NATS").with_source(e))
    }

    /// Loads authentication keys from configured paths.
    pub async fn load_auth_keys(&self) -> Result<SessionKeys> {
        let config = AuthKeysConfig::new(&self.auth_decoding_key, &self.auth_encoding_key);
        SessionKeys::from_config(config).await
    }
}

impl ServiceConfigBuilder {
    /// Wrapper for builder validation that returns String errors.
    fn validate(builder: &ServiceConfigBuilder) -> Result<(), String> {
        // Validate postgres connection URL format
        if let Some(endpoint) = &builder.postgres_endpoint {
            if endpoint.is_empty() {
                return Err("Postgres connection URL cannot be empty".to_string());
            }

            if !endpoint.starts_with("postgresql://") && !endpoint.starts_with("postgres://") {
                return Err(
                    "Postgres connection URL must start with 'postgresql://' or 'postgres://'"
                        .to_string(),
                );
            }
        }

        // Validate OpenRouter API key
        if let Some(api_key) = &builder.openrouter_api_key
            && api_key.is_empty()
        {
            return Err("OpenRouter API key cannot be empty".to_string());
        }

        // Validate NATS URL
        if let Some(nats_url) = &builder.nats_url {
            if nats_url.is_empty() {
                return Err("NATS URL cannot be empty".to_string());
            }

            if !nats_url.starts_with("nats://") && !nats_url.starts_with("tls://") {
                return Err("NATS URL must start with 'nats://' or 'tls://'".to_string());
            }
        }

        // Validate postgres max connections
        if let Some(max_connections) = &builder.postgres_max_connections {
            if *max_connections == 0 {
                return Err("Postgres max connections must be greater than 0".to_string());
            }
            if *max_connections > 16 {
                return Err("Postgres max connections cannot exceed 16".to_string());
            }
        }

        // Validate postgres connection timeout
        if let Some(timeout_secs) = &builder.postgres_connection_timeout_secs {
            if *timeout_secs < 1 {
                return Err("Postgres connection timeout must be at least 1 second".to_string());
            }
            if *timeout_secs > 300 {
                return Err("Postgres connection timeout cannot exceed 300 seconds".to_string());
            }
        }

        Ok(())
    }
}

#[cfg(debug_assertions)]
impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            postgres_endpoint: defaults::POSTGRES_ENDPOINT.to_string(),
            postgres_max_connections: defaults::POSTGRES_MAX_CONNECTIONS,
            postgres_connection_timeout_secs: defaults::POSTGRES_CONNECTION_TIMEOUT_SECS,
            auth_decoding_key: defaults::auth_decoding_key(),
            auth_encoding_key: defaults::auth_encoding_key(),
            openrouter_api_key: defaults::openrouter_api_key(),
            openrouter_base_url: None,
            nats_url: defaults::NATS_URL.to_string(),
        }
    }
}
