use std::path::PathBuf;

use anyhow::{Result as AnyhowResult, anyhow};
use nvisy_minio::MinioClient;
use nvisy_openrouter::{LlmConfig, OpenRouter};
use nvisy_postgres::{PgConfig, PgDatabase};
use serde::{Deserialize, Serialize};

use crate::service::auth::{AuthHasher, AuthKeys, AuthKeysConfig};
use crate::service::policy::RegionalPolicy;
use crate::service::{Result, ServiceError};

/// Stripe client placeholder.
///
/// TODO: Implement actual Stripe integration.
#[derive(Debug, Clone)]
pub struct Stripe;

impl Stripe {
    /// Creates a new Stripe client with the given API key.
    ///
    /// # Errors
    ///
    /// Returns an error if the API key is invalid.
    pub fn new(_api_key: &str) -> Result<Self> {
        // TODO: Implement actual Stripe client initialization
        Ok(Self)
    }
}

/// App [`state`] configuration.
///
/// [`state`]: crate::service::ServiceState
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(not(debug_assertions), derive(clap::Args))]
#[cfg_attr(debug_assertions, derive(clap::Parser))]
#[must_use = "config does nothing unless you use it"]
pub struct ServiceConfig {
    /// Postgres database connection string.
    #[arg(short = 'd', long, env = "POSTGRES_URL")]
    #[arg(default_value = "postgresql://postgres:postgres@localhost:5432/postgres")]
    pub postgres_url: String,

    // Controls the regional policy used for data collection.
    #[arg(short = 'r', long, env = "DATA_COLLECTION_POLICY")]
    #[arg(default_value_t = true)]
    pub minimal_data_collection: bool,

    /// File path to the JWT decoding (public) key used for sessions.
    #[arg(long, env = "AUTH_PUBLIC_PEM_FILEPATH")]
    #[arg(default_value = "./public.pem")]
    pub auth_decoding_key: PathBuf,

    /// File path to the JWT encode (private) key used for sessions.
    #[arg(long, env = "AUTH_PRIVATE_PEM_FILEPATH")]
    #[arg(default_value = "./private.pem")]
    pub auth_encoding_key: PathBuf,

    /// OpenRouter API key.
    #[arg(long, env = "OPENROUTER_API_KEY")]
    pub openrouter_api_key: String,

    /// OpenRouter base URL.
    #[arg(long, env = "OPENROUTER_BASE_URL")]
    #[arg(default_value = "https://openrouter.ai/api/v1/")]
    pub openrouter_base_url: Option<String>,

    /// Stripe API key.
    #[arg(long, env = "STRIPE_API_KEY")]
    pub stripe_api_key: String,

    /// MinIO endpoint URL.
    #[arg(long, env = "MINIO_ENDPOINT")]
    #[arg(default_value = "localhost:9000")]
    pub minio_endpoint: String,

    /// MinIO access key.
    #[arg(long, env = "MINIO_ACCESS_KEY")]
    #[arg(default_value = "minioadmin")]
    pub minio_access_key: String,

    /// MinIO secret key.
    #[arg(long, env = "MINIO_SECRET_KEY")]
    #[arg(default_value = "minioadmin")]
    pub minio_secret_key: String,
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
        if self.postgres_url.is_empty() {
            return Err(anyhow!("Postgres connection URL cannot be empty"));
        }

        if !self.postgres_url.starts_with("postgresql://")
            && !self.postgres_url.starts_with("postgres://")
        {
            return Err(anyhow!(
                "Postgres connection URL must start with 'postgresql://' or 'postgres://'"
            ));
        }

        // Validate OpenRouter API key
        if self.openrouter_api_key.is_empty() {
            return Err(anyhow!("OpenRouter API key cannot be empty"));
        }

        // Validate Stripe API key
        if self.stripe_api_key.is_empty() {
            return Err(anyhow!("Stripe API key cannot be empty"));
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

        Ok(())
    }

    /// Connects to Postgres database and runs migrations.
    pub async fn connect_postgres(&self) -> Result<PgDatabase> {
        use nvisy_postgres::migrate::PgDatabaseExt;

        let pool_config = nvisy_postgres::PgPoolConfig::default();
        let config = PgConfig::new(self.postgres_url.clone(), pool_config);
        let pg_database = PgDatabase::new(config).map_err(|e| {
            ServiceError::database_with_source("Failed to create database client", e)
        })?;

        pg_database.run_pending_migrations().await.map_err(|e| {
            ServiceError::database_with_source("Failed to apply database migrations", e)
        })?;

        Ok(pg_database)
    }

    /// Connects to OpenRouter LLM service.
    #[inline]
    pub async fn connect_openrouter(&self) -> Result<OpenRouter> {
        let config = match &self.openrouter_base_url {
            None => LlmConfig::default(),
            Some(base_url) => LlmConfig::default().with_base_url(base_url.clone()),
        };

        OpenRouter::from_api_key_with_config(&self.openrouter_api_key, config).map_err(|e| {
            ServiceError::external_service_with_source(
                "OpenRouter",
                "Failed to initialize client",
                e,
            )
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

    /// Connects to Stripe payment service.
    #[inline]
    pub async fn connect_stripe(&self) -> Result<Stripe> {
        Stripe::new(&self.stripe_api_key)
    }

    /// Returns the configured regional data collection policy.
    #[inline]
    pub const fn regional_policy(&self) -> RegionalPolicy {
        if self.minimal_data_collection {
            RegionalPolicy::MinimalDataCollection
        } else {
            RegionalPolicy::NormalDataCollection
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
            postgres_url: "postgresql://postgres:postgres@localhost:5432/postgres".to_owned(),
            minimal_data_collection: true,
            auth_decoding_key: "./public.pem".into(),
            auth_encoding_key: "./private.pem".into(),
            openrouter_api_key: format!("sk-or-v1-{}", "A".repeat(64)),
            openrouter_base_url: None,
            stripe_api_key: format!("rk_{}", "A".repeat(100)),
            minio_endpoint: "localhost:9000".to_owned(),
            minio_access_key: "minioadmin".to_owned(),
            minio_secret_key: "minioadmin".to_owned(),
        }
    }
}
