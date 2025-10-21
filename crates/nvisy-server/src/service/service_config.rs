use std::path::PathBuf;

use anyhow::{Result as AnyhowResult, anyhow};
use nvisy_minio::MinioClient;
use nvisy_postgres::client::PgClientExt;
use nvisy_postgres::{PgClient, PgConfig};
use serde::{Deserialize, Serialize};

use crate::service::auth::{AuthHasher, AuthKeys, AuthKeysConfig};
use crate::service::policy::RegionalPolicy;
use crate::service::{Result, ServiceError};

/// App [`state`] configuration.
///
/// [`state`]: crate::service::ServiceState
#[derive(Debug, Clone, Serialize, Deserialize)]
#[must_use = "config does nothing unless you use it"]
pub struct ServiceConfig {
    /// Postgres database connection string.
    pub postgres_endpoint: String,

    // Controls the regional policy used for data collection.
    pub minimal_data_collection: bool,

    /// File path to the JWT decoding (public) key used for sessions.
    pub auth_decoding_key: PathBuf,

    /// File path to the JWT encode (private) key used for sessions.
    pub auth_encoding_key: PathBuf,

    /// OpenRouter API key.
    pub openrouter_api_key: String,

    /// OpenRouter base URL.
    pub openrouter_base_url: Option<String>,

    /// MinIO endpoint URL.
    pub minio_endpoint: String,

    /// MinIO access key.
    pub minio_access_key: String,

    /// MinIO secret key.
    pub minio_secret_key: String,

    /// NATS server URL.
    pub nats_url: String,

    /// NATS client name.
    pub nats_client_name: String,
}

impl ServiceConfig {
    /// Validates all configuration values and returns errors for invalid settings.
    ///
    /// # Errors
    ///
    /// Returns an error if any configuration value is invalid:
    /// - Postgres connection URL must be a valid format
    /// - Auth key files must exist
    /// - API keys must not be empty
    /// - OpenRouter base URL must be valid if provided
    pub fn validate(&self) -> AnyhowResult<()> {
        // Validate postgres connection URL format
        if self.postgres_endpoint.is_empty() {
            return Err(anyhow!("Postgres connection URL cannot be empty"));
        }

        if !self.postgres_endpoint.starts_with("postgresql://")
            && !self.postgres_endpoint.starts_with("postgres://")
        {
            return Err(anyhow!(
                "Postgres connection URL must start with 'postgresql://' or 'postgres://'"
            ));
        }

        // Validate OpenRouter API key
        if self.openrouter_api_key.is_empty() {
            return Err(anyhow!("OpenRouter API key cannot be empty"));
        }

        // Validate MinIO endpoint
        if self.minio_endpoint.is_empty() {
            return Err(anyhow!("MinIO endpoint cannot be empty"));
        }

        // Validate MinIO access key
        if self.minio_access_key.is_empty() {
            return Err(anyhow!("MinIO access key cannot be empty"));
        }

        // Validate MinIO secret key
        if self.minio_secret_key.is_empty() {
            return Err(anyhow!("MinIO secret key cannot be empty"));
        }

        // Validate NATS URL
        if self.nats_url.is_empty() {
            return Err(anyhow!("NATS URL cannot be empty"));
        }

        if !self.nats_url.starts_with("nats://") && !self.nats_url.starts_with("tls://") {
            return Err(anyhow!("NATS URL must start with 'nats://' or 'tls://'"));
        }

        Ok(())
    }

    /// Connects to Postgres database and runs migrations.
    pub async fn connect_postgres(&self) -> Result<PgClient> {
        let pool_config = nvisy_postgres::PgPoolConfig::default();
        let config = PgConfig::new(self.postgres_endpoint.clone(), pool_config);
        let pg_client = PgClient::new(config).map_err(|e| {
            ServiceError::database_with_source("Failed to create database client", e)
        })?;

        pg_client.run_pending_migrations().await.map_err(|e| {
            ServiceError::database_with_source("Failed to apply database migrations", e)
        })?;

        Ok(pg_client)
    }

    /// Connects to OpenRouter LLM service.
    /// TODO: Implement when nvisy-openrouter is fully available
    #[inline]
    pub async fn connect_openrouter(&self) -> Result<()> {
        // Placeholder until nvisy-openrouter is implemented
        tracing::warn!("OpenRouter connection not yet implemented");
        Ok(())
    }

    /// Connects to NATS server.
    #[inline]
    pub async fn connect_nats(&self) -> Result<nvisy_nats::NatsClient> {
        use nvisy_nats::NatsConfig;

        let config = NatsConfig::new(&self.nats_url).with_name(&self.nats_client_name);

        nvisy_nats::NatsClient::connect(config).await.map_err(|e| {
            ServiceError::external_service_with_source("NATS", "Failed to connect to NATS", e)
        })
    }

    /// Connects to MinIO file storage.
    #[inline]
    pub async fn connect_file_storage(&self) -> Result<MinioClient> {
        use nvisy_minio::{MinioConfig, MinioCredentials};

        let credentials =
            MinioCredentials::new(self.minio_access_key.clone(), self.minio_secret_key.clone());

        // Parse endpoint URL - MinioConfig enforces HTTPS
        let endpoint_url = format!("https://{}", self.minio_endpoint);

        let url = endpoint_url
            .parse::<url::Url>()
            .map_err(|e| ServiceError::config_with_source("Invalid MinIO endpoint URL", e))?;

        let config = MinioConfig::new(url, credentials)
            .map_err(|e| ServiceError::config_with_source("Failed to create MinIO config", e))?;

        MinioClient::new(config).map_err(|e| {
            ServiceError::external_service_with_source("MinIO", "Failed to connect to MinIO", e)
        })
    }

    /// Returns the configured regional data collection policy.
    #[inline]
    pub const fn regional_policy(&self) -> RegionalPolicy {
        if self.minimal_data_collection {
            RegionalPolicy::minimal()
        } else {
            RegionalPolicy::normal()
        }
    }

    /// Loads authentication keys from configured paths.
    pub async fn load_auth_keys(&self) -> Result<AuthKeys> {
        let config = AuthKeysConfig::new(&self.auth_decoding_key, &self.auth_encoding_key);
        AuthKeys::from_config(config).await
    }

    /// Creates a password hasher with secure defaults.
    pub fn create_password_hasher(&self) -> Result<AuthHasher> {
        AuthHasher::new()
    }
}

#[cfg(debug_assertions)]
impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            postgres_endpoint: "postgresql://postgres:postgres@localhost:5432/postgres".to_owned(),
            minimal_data_collection: true,
            auth_decoding_key: "./public.pem".into(),
            auth_encoding_key: "./private.pem".into(),
            openrouter_api_key: format!("sk-or-v1-{}", "A".repeat(64)),
            openrouter_base_url: None,
            minio_endpoint: "localhost:9000".to_owned(),
            minio_access_key: "minioadmin".to_owned(),
            minio_secret_key: "minioadmin".to_owned(),
            nats_url: "nats://127.0.0.1:4222".to_owned(),
            nats_client_name: "nvisy-api".to_owned(),
        }
    }
}
