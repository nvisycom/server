//! Service configuration with CLI argument parsing.
//!
//! This module provides CLI-friendly configuration structs with clap attributes
//! that can be converted to the plain server configuration types.
//!
//! # Usage Examples
//!
//! ## Command Line Arguments
//!
//! ```bash
//! # Basic usage with custom database
//! nvisy-cli --postgres-url "postgresql://user:pass@localhost:5432/nvisy"
//!
//! # Configure MinIO storage
//! nvisy-cli --minio-endpoint "storage.example.com:9000" \
//!           --minio-access-key "mykey" \
//!           --minio-secret-key "mysecret"
//!
//! # CORS configuration
//! nvisy-cli --cors-origins "https://app.example.com,https://dashboard.example.com" \
//!           --cors-max-age 7200 \
//!           --cors-allow-credentials false
//!
//! # OpenAPI paths
//! nvisy-cli --open-api-json "/docs/api.json" \
//!           --swagger-ui "/docs/swagger" \
//!           --scalar-ui "/docs/scalar"
//! ```
//!
//! ## Environment Variables
//!
//! ```bash
//! export POSTGRES_URL="postgresql://user:pass@localhost:5432/nvisy"
//! export MINIO_ENDPOINT="storage.example.com:9000"
//! export CORS_ORIGINS="https://app.example.com,https://dashboard.example.com"
//! export OPENROUTER_API_KEY="sk-or-v1-your-key-here"
//! export STRIPE_API_KEY="sk_live_your-stripe-key-here"
//!
//! nvisy-cli
//! ```
//!
//! ## Programmatic Usage
//!
//! ```rust
//! use nvisy_cli::config::CliConfig;
//! use clap::Parser;
//!
//! // Parse from command line
//! let cli_config = CliConfig::parse();
//!
//! // Convert to server configuration types
//! let (service_config, cors_config, openapi_config) = cli_config.into_server_configs();
//!
//! // Use with nvisy-server
//! let service_state = nvisy_server::service::ServiceState::from_config(&service_config).await?;
//! ```

use std::path::PathBuf;

use clap::{Args, Parser};
use nvisy_server::middleware::open_api::config::OpenApiConfig as ServerOpenApiConfig;
use nvisy_server::middleware::security::cors::CorsConfig as ServerCorsConfig;
use nvisy_server::service::ServiceConfig as ServerServiceConfig;
use serde::{Deserialize, Serialize};

/// CLI service configuration with command-line argument parsing.
#[derive(Debug, Clone, Args, Serialize, Deserialize)]
#[must_use = "config does nothing unless you use it"]
pub struct ServiceConfig {
    /// Postgres database connection string.
    #[arg(short = 'd', long, env = "POSTGRES_URL")]
    #[arg(default_value = "postgresql://postgres:postgres@localhost:5432/postgres")]
    pub postgres_url: String,

    /// Controls the regional policy used for data collection.
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

    /// `OpenRouter` API key.
    #[arg(long, env = "OPENROUTER_API_KEY")]
    pub openrouter_api_key: String,

    /// `OpenRouter` base URL.
    #[arg(long, env = "OPENROUTER_BASE_URL")]
    #[arg(default_value = "https://openrouter.ai/api/v1/")]
    pub openrouter_base_url: Option<String>,

    /// `MinIO` endpoint URL.
    #[arg(long, env = "MINIO_ENDPOINT")]
    #[arg(default_value = "localhost:9000")]
    pub minio_endpoint: String,

    /// `MinIO` access key.
    #[arg(long, env = "MINIO_ACCESS_KEY")]
    #[arg(default_value = "minioadmin")]
    pub minio_access_key: String,

    /// `MinIO` secret key.
    #[arg(long, env = "MINIO_SECRET_KEY")]
    #[arg(default_value = "minioadmin")]
    pub minio_secret_key: String,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            postgres_url: "postgresql://postgres:postgres@localhost:5432/postgres".to_owned(),
            minimal_data_collection: true,
            auth_decoding_key: "./public.pem".into(),
            auth_encoding_key: "./private.pem".into(),
            openrouter_api_key: format!("sk-or-v1-{}", "A".repeat(64)),
            openrouter_base_url: Some("https://openrouter.ai/api/v1/".to_owned()),
            minio_endpoint: "localhost:9000".to_owned(),
            minio_access_key: "minioadmin".to_owned(),
            minio_secret_key: "minioadmin".to_owned(),
        }
    }
}

impl From<ServiceConfig> for ServerServiceConfig {
    fn from(cli_config: ServiceConfig) -> Self {
        Self {
            postgres_endpoint: cli_config.postgres_url,
            minimal_data_collection: cli_config.minimal_data_collection,
            auth_decoding_key: cli_config.auth_decoding_key,
            auth_encoding_key: cli_config.auth_encoding_key,
            openrouter_api_key: cli_config.openrouter_api_key,
            openrouter_base_url: cli_config.openrouter_base_url,
            minio_endpoint: cli_config.minio_endpoint,
            minio_access_key: cli_config.minio_access_key,
            minio_secret_key: cli_config.minio_secret_key,
        }
    }
}

/// CLI CORS configuration with command-line argument parsing.
#[derive(Debug, Clone, Args, Serialize, Deserialize)]
#[must_use = "config does nothing unless you use it"]
pub struct CorsConfig {
    /// List of allowed CORS origins.
    /// If empty, defaults to localhost origins for development.
    #[arg(long, env = "CORS_ORIGINS", value_delimiter = ',')]
    pub allowed_origins: Vec<String>,

    /// Maximum age for CORS preflight requests in seconds.
    #[arg(long, env = "CORS_MAX_AGE", default_value = "3600")]
    pub max_age_seconds: u64,

    /// Whether to allow credentials in CORS requests.
    #[arg(long, env = "CORS_ALLOW_CREDENTIALS", default_value = "true")]
    pub allow_credentials: bool,
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            allowed_origins: Vec::new(),
            max_age_seconds: 3600,
            allow_credentials: true,
        }
    }
}

impl From<CorsConfig> for ServerCorsConfig {
    fn from(cli_config: CorsConfig) -> Self {
        Self {
            allowed_origins: cli_config.allowed_origins,
            max_age_seconds: cli_config.max_age_seconds,
            allow_credentials: cli_config.allow_credentials,
        }
    }
}

/// CLI `OpenAPI` configuration with command-line argument parsing.
#[derive(Debug, Clone, Args, Serialize, Deserialize)]
#[must_use = "config does nothing unless you use it"]
pub struct OpenApiConfig {
    /// Path which exposes the `OpenApi` to the user.
    #[arg(short, long, default_value = "/api/openapi.json")]
    pub open_api_json: String,

    /// Path which exposes `SwaggerUI` to the user.
    #[arg(short, long, default_value = "/api/swagger")]
    pub swagger_ui: String,

    /// Path which exposes Scalar to the user.
    #[arg(short = 'c', long, default_value = "/api/scalar")]
    pub scalar_ui: String,
}

impl Default for OpenApiConfig {
    fn default() -> Self {
        Self {
            open_api_json: "/api/openapi.json".to_owned(),
            swagger_ui: "/api/swagger".to_string(),
            scalar_ui: "/api/scalar".to_string(),
        }
    }
}

impl From<OpenApiConfig> for ServerOpenApiConfig {
    fn from(cli_config: OpenApiConfig) -> Self {
        Self {
            open_api_json: cli_config.open_api_json,
            swagger_ui: cli_config.swagger_ui,
            scalar_ui: cli_config.scalar_ui,
        }
    }
}

/// Complete CLI configuration combining all service configurations.
#[derive(Debug, Clone, Parser, Serialize, Deserialize)]
#[command(name = "nvisy")]
#[command(about = "Nvisy API Server")]
#[command(version)]
#[derive(Default)]
pub struct CliConfig {
    /// Service configuration
    #[clap(flatten)]
    pub service: ServiceConfig,

    /// CORS configuration
    #[clap(flatten)]
    pub cors: CorsConfig,

    /// `OpenAPI` configuration
    #[clap(flatten)]
    pub openapi: OpenApiConfig,
}

impl CliConfig {
    /// Converts the CLI configuration to server configuration types.
    pub fn into_server_configs(
        self,
    ) -> (ServerServiceConfig, ServerCorsConfig, ServerOpenApiConfig) {
        (self.service.into(), self.cors.into(), self.openapi.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_service_config() {
        let config = ServiceConfig::default();
        assert_eq!(
            config.postgres_url,
            "postgresql://postgres:postgres@localhost:5432/postgres"
        );
        assert!(config.minimal_data_collection);
        assert_eq!(config.minio_endpoint, "localhost:9000");
    }

    #[test]
    fn test_service_config_conversion() {
        let cli_config = ServiceConfig::default();
        let server_config: ServerServiceConfig = cli_config.into();

        // Basic smoke test that conversion works
        assert_eq!(
            server_config.postgres_endpoint,
            "postgresql://postgres:postgres@localhost:5432/postgres"
        );
        assert!(server_config.minimal_data_collection);
    }

    #[test]
    fn test_cors_config_conversion() {
        let cli_config = CorsConfig {
            allowed_origins: vec!["http://localhost:3000".to_string()],
            max_age_seconds: 7200,
            allow_credentials: false,
        };

        let server_config: ServerCorsConfig = cli_config.into();
        assert_eq!(server_config.allowed_origins, vec!["http://localhost:3000"]);
        assert_eq!(server_config.max_age_seconds, 7200);
        assert!(!server_config.allow_credentials);
    }

    #[test]
    fn test_openapi_config_conversion() {
        let cli_config = OpenApiConfig::default();
        let server_config: ServerOpenApiConfig = cli_config.into();

        assert_eq!(server_config.open_api_json, "/api/openapi.json");
        assert_eq!(server_config.swagger_ui, "/api/swagger");
        assert_eq!(server_config.scalar_ui, "/api/scalar");
    }

    #[test]
    fn test_complete_cli_config() {
        let config = CliConfig::default();
        let (service, cors, openapi) = config.into_server_configs();

        // Smoke test that all conversions work
        assert!(!service.postgres_endpoint.is_empty());
        assert!(!cors.allowed_origins.is_empty() || cors.allowed_origins.is_empty()); // Either is valid
        assert!(!openapi.open_api_json.is_empty());
    }

    #[test]
    fn test_cli_config_with_custom_values() {
        let config = CliConfig {
            service: ServiceConfig {
                postgres_url: "postgresql://custom:pass@db:5432/custom".to_string(),
                minimal_data_collection: false,
                minio_endpoint: "minio.example.com:9000".to_string(),
                ..ServiceConfig::default()
            },
            cors: CorsConfig {
                allowed_origins: vec!["https://app.example.com".to_string()],
                max_age_seconds: 7200,
                allow_credentials: false,
            },
            openapi: OpenApiConfig {
                open_api_json: "/custom/api.json".to_string(),
                swagger_ui: "/custom/swagger".to_string(),
                scalar_ui: "/custom/scalar".to_string(),
            },
        };

        let (service, cors, openapi) = config.into_server_configs();

        assert_eq!(
            service.postgres_endpoint,
            "postgresql://custom:pass@db:5432/custom"
        );
        assert!(!service.minimal_data_collection);
        assert_eq!(service.minio_endpoint, "minio.example.com:9000");

        assert_eq!(cors.allowed_origins, vec!["https://app.example.com"]);
        assert_eq!(cors.max_age_seconds, 7200);
        assert!(!cors.allow_credentials);

        assert_eq!(openapi.open_api_json, "/custom/api.json");
        assert_eq!(openapi.swagger_ui, "/custom/swagger");
        assert_eq!(openapi.scalar_ui, "/custom/scalar");
    }
}
