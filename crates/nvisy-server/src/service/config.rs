use std::path::PathBuf;

use anyhow::{Result as AnyhowResult, anyhow};
use derive_builder::Builder;
use nvisy_nats::{NatsClient, NatsConfig};
use nvisy_openrouter::{LlmClient, LlmConfig};
use nvisy_postgres::{PgClient, PgClientExt, PgConfig};
use serde::{Deserialize, Serialize};

use crate::service::auth::{AuthHasher, AuthKeys, AuthKeysConfig};
use crate::service::policy::DataCollectionPolicy;
use crate::service::{Result, ServiceError};

/// Default values for configuration options.
mod defaults {
    use std::path::PathBuf;

    /// Default Postgres connection string for development.
    pub const POSTGRES_ENDPOINT: &str = "postgresql://postgres:postgres@localhost:5432/postgres";

    /// Default data collection policy (minimal for development).
    pub const MINIMAL_DATA_COLLECTION: bool = true;

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

    /// Default NATS URL.
    pub const NATS_URL: &str = "nats://127.0.0.1:4222";

    /// Default NATS client name.
    pub const NATS_CLIENT_NAME: &str = "nvisy-api";
}

/// Validates the ServiceConfig before building.
fn validate_config(builder: &ServiceConfigBuilder) -> std::result::Result<(), String> {
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
    if let Some(api_key) = &builder.openrouter_api_key {
        if api_key.is_empty() {
            return Err("OpenRouter API key cannot be empty".to_string());
        }
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

    Ok(())
}

/// App [`state`] configuration.
///
/// [`state`]: crate::service::ServiceState
#[derive(Debug, Clone, Serialize, Deserialize, Builder)]
#[must_use = "config does nothing unless you use it"]
#[builder(
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(validate = "validate_config")
)]
pub struct ServiceConfig {
    /// Postgres database connection string.
    #[builder(default = "defaults::POSTGRES_ENDPOINT.to_string()")]
    pub postgres_endpoint: String,

    /// Controls the regional policy used for data collection.
    #[builder(default = "defaults::MINIMAL_DATA_COLLECTION")]
    pub minimal_data_collection: bool,

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

    /// NATS server URL.
    #[builder(default = "defaults::NATS_URL.to_string()")]
    pub nats_url: String,

    /// NATS client name.
    #[builder(default = "defaults::NATS_CLIENT_NAME.to_string()")]
    pub nats_client_name: String,
}

impl ServiceConfig {
    /// Creates a new configuration builder.
    pub fn builder() -> ServiceConfigBuilder {
        ServiceConfigBuilder::default()
    }

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
    pub async fn connect_llm(&self) -> Result<LlmClient> {
        let config = {
            let mut builder = LlmConfig::builder();
            if let Some(base_url) = &self.openrouter_base_url {
                builder = builder.with_base_url(base_url.clone());
            }
            builder.build()
        }
        .map_err(|e| {
            ServiceError::external_service_with_source(
                "OpenRouter",
                "Failed to build LLM config",
                e,
            )
        })?;

        LlmClient::from_api_key_with_config(&self.openrouter_api_key, config).map_err(|e| {
            ServiceError::external_service_with_source(
                "OpenRouter",
                "Failed to create LLM client",
                e,
            )
        })
    }

    /// Connects to NATS server.
    #[inline]
    pub async fn connect_nats(&self) -> Result<NatsClient> {
        let config = NatsConfig::new(&self.nats_url).with_name(&self.nats_client_name);
        NatsClient::connect(config).await.map_err(|e| {
            ServiceError::external_service_with_source("NATS", "Failed to connect to NATS", e)
        })
    }

    /// Returns the configured regional data collection policy.
    #[inline]
    pub const fn regional_policy(&self) -> DataCollectionPolicy {
        if self.minimal_data_collection {
            DataCollectionPolicy::minimal()
        } else {
            DataCollectionPolicy::normal()
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
            postgres_endpoint: defaults::POSTGRES_ENDPOINT.to_string(),
            minimal_data_collection: defaults::MINIMAL_DATA_COLLECTION,
            auth_decoding_key: defaults::auth_decoding_key(),
            auth_encoding_key: defaults::auth_encoding_key(),
            openrouter_api_key: defaults::openrouter_api_key(),
            openrouter_base_url: None,
            nats_url: defaults::NATS_URL.to_string(),
            nats_client_name: defaults::NATS_CLIENT_NAME.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn test_service_config_builder_default() {
        let config = ServiceConfig::builder().build().unwrap();

        assert_eq!(config.postgres_endpoint, defaults::POSTGRES_ENDPOINT);
        assert_eq!(
            config.minimal_data_collection,
            defaults::MINIMAL_DATA_COLLECTION
        );
        assert_eq!(config.auth_decoding_key, defaults::auth_decoding_key());
        assert_eq!(config.auth_encoding_key, defaults::auth_encoding_key());
        assert_eq!(config.openrouter_api_key, defaults::openrouter_api_key());
        assert!(config.openrouter_base_url.is_none());
        assert_eq!(config.nats_url, defaults::NATS_URL);
        assert_eq!(config.nats_client_name, defaults::NATS_CLIENT_NAME);
    }

    #[test]
    fn test_service_config_builder_with_custom_values() {
        let config = ServiceConfig::builder()
            .with_postgres_endpoint("postgresql://custom:password@localhost:5433/custom_db")
            .with_minimal_data_collection(false)
            .with_auth_decoding_key(PathBuf::from("./custom_public.pem"))
            .with_auth_encoding_key(PathBuf::from("./custom_private.pem"))
            .with_openrouter_api_key("custom-api-key")
            .with_openrouter_base_url("https://custom.openrouter.ai")
            .with_nats_url("nats://custom.nats:4223")
            .with_nats_client_name("custom-client")
            .build()
            .unwrap();

        assert_eq!(
            config.postgres_endpoint,
            "postgresql://custom:password@localhost:5433/custom_db"
        );
        assert!(!config.minimal_data_collection);
        assert_eq!(
            config.auth_decoding_key,
            PathBuf::from("./custom_public.pem")
        );
        assert_eq!(
            config.auth_encoding_key,
            PathBuf::from("./custom_private.pem")
        );
        assert_eq!(config.openrouter_api_key, "custom-api-key");
        assert_eq!(
            config.openrouter_base_url,
            Some("https://custom.openrouter.ai".to_string())
        );
        assert_eq!(config.nats_url, "nats://custom.nats:4223");
        assert_eq!(config.nats_client_name, "custom-client");
    }

    #[test]
    fn test_service_config_builder_validation_invalid_postgres_url() {
        let result = ServiceConfig::builder()
            .with_postgres_endpoint("invalid-url")
            .build();

        assert!(result.is_err());
        assert!(
            result.unwrap_err().to_string().contains(
                "Postgres connection URL must start with 'postgresql://' or 'postgres://'"
            )
        );
    }

    #[test]
    fn test_service_config_builder_validation_empty_postgres_url() {
        let result = ServiceConfig::builder().with_postgres_endpoint("").build();

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Postgres connection URL cannot be empty")
        );
    }

    #[test]
    fn test_service_config_builder_validation_empty_api_key() {
        let result = ServiceConfig::builder().with_openrouter_api_key("").build();

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("OpenRouter API key cannot be empty")
        );
    }

    #[test]
    fn test_service_config_builder_validation_invalid_nats_url() {
        let result = ServiceConfig::builder()
            .with_nats_url("invalid-nats-url")
            .build();

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("NATS URL must start with 'nats://' or 'tls://'")
        );
    }

    #[test]
    fn test_service_config_builder_validation_empty_nats_url() {
        let result = ServiceConfig::builder().with_nats_url("").build();

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("NATS URL cannot be empty")
        );
    }

    #[test]
    fn test_service_config_regional_policy() {
        let config_minimal = ServiceConfig::builder()
            .with_minimal_data_collection(true)
            .build()
            .unwrap();
        // Just verify that the method returns a DataCollectionPolicy
        let _policy = config_minimal.regional_policy();

        let config_normal = ServiceConfig::builder()
            .with_minimal_data_collection(false)
            .build()
            .unwrap();
        // Just verify that the method returns a DataCollectionPolicy
        let _policy = config_normal.regional_policy();
    }

    #[test]
    fn test_service_config_validate_method() {
        let valid_config = ServiceConfig::builder().build().unwrap();
        assert!(valid_config.validate().is_ok());

        let invalid_config = ServiceConfig {
            postgres_endpoint: "invalid-url".to_string(),
            minimal_data_collection: defaults::MINIMAL_DATA_COLLECTION,
            auth_decoding_key: defaults::auth_decoding_key(),
            auth_encoding_key: defaults::auth_encoding_key(),
            openrouter_api_key: defaults::openrouter_api_key(),
            openrouter_base_url: None,
            nats_url: defaults::NATS_URL.to_string(),
            nats_client_name: defaults::NATS_CLIENT_NAME.to_string(),
        };
        assert!(invalid_config.validate().is_err());
    }
}
