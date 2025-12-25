//! Service configuration with CLI argument parsing.
//!
//! This module provides CLI-friendly configuration structs with clap attributes
//! that can be converted to the plain server configuration types.

use std::path::PathBuf;

use clap::{Args, Parser};
use nvisy_nats::NatsConfig;
use nvisy_postgres::PgConfig;
use nvisy_qdrant::QdrantConfig;
use nvisy_server::middleware::{
    CorsConfig as ServerCorsConfig, OpenApiConfig as ServerOpenApiConfig,
};
use nvisy_server::service::ServiceConfig as ServerServiceConfig;
use serde::{Deserialize, Serialize};

/// CLI service configuration with command-line argument parsing.
#[derive(Debug, Clone, Args, Serialize, Deserialize)]
#[must_use = "config does nothing unless you use it"]
pub struct ServiceConfig {
    /// Postgres configuration.
    #[clap(flatten)]
    pub postgres: PgConfig,

    /// NATS configuration.
    #[clap(flatten)]
    pub nats: NatsConfig,

    /// Qdrant configuration.
    #[clap(flatten)]
    pub qdrant: QdrantConfig,

    /// File path to the JWT decoding (public) key used for sessions.
    #[arg(long, env = "AUTH_PUBLIC_PEM_FILEPATH")]
    #[arg(default_value = "./public.pem")]
    pub auth_decoding_key: PathBuf,

    /// File path to the JWT encode (private) key used for sessions.
    #[arg(long, env = "AUTH_PRIVATE_PEM_FILEPATH")]
    #[arg(default_value = "./private.pem")]
    pub auth_encoding_key: PathBuf,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            postgres: PgConfig::default(),
            nats: NatsConfig::default(),
            qdrant: QdrantConfig::default(),
            auth_decoding_key: "./public.pem".into(),
            auth_encoding_key: "./private.pem".into(),
        }
    }
}

impl From<ServiceConfig> for ServerServiceConfig {
    fn from(cli_config: ServiceConfig) -> Self {
        Self {
            postgres_config: cli_config.postgres,
            nats_config: cli_config.nats,
            qdrant_config: cli_config.qdrant,
            auth_decoding_key: cli_config.auth_decoding_key,
            auth_encoding_key: cli_config.auth_encoding_key,
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

    /// Path which exposes Scalar to the user.
    #[arg(short = 'c', long, default_value = "/api/scalar")]
    pub scalar_ui: String,
}

impl Default for OpenApiConfig {
    fn default() -> Self {
        Self {
            open_api_json: "/api/openapi.json".to_owned(),
            scalar_ui: "/api/scalar".to_string(),
        }
    }
}

impl From<OpenApiConfig> for ServerOpenApiConfig {
    fn from(cli_config: OpenApiConfig) -> Self {
        Self {
            open_api_json: cli_config.open_api_json,
            scalar_ui: cli_config.scalar_ui,
        }
    }
}

/// Complete CLI configuration combining all service configurations.
#[derive(Debug, Clone, Parser, Serialize, Deserialize, Default)]
#[command(name = "nvisy")]
#[command(about = "Nvisy API Server")]
#[command(version)]
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
