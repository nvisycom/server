//! Service configuration with CLI argument parsing.
//!
//! This module provides CLI-friendly configuration structs with clap attributes
//! that can be converted to the plain server configuration types.

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

    /// Maximum number of connections in the Postgres connection pool.
    #[arg(long, env = "POSTGRES_MAX_CONNECTIONS")]
    #[arg(default_value_t = 10)]
    pub postgres_max_pool: u32,

    /// Connection timeout for Postgres operations in seconds.
    #[arg(long, env = "POSTGRES_CONNECTION_TIMEOUT_SECS")]
    #[arg(default_value_t = 30)]
    pub postgres_timeout_secs: u64,

    /// NATS server URL.
    #[arg(long, env = "NATS_URL")]
    #[arg(default_value = "nats://127.0.0.1:4222")]
    pub nats_url: String,

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
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            postgres_url: "postgresql://postgres:postgres@localhost:5432/postgres".to_owned(),
            postgres_max_pool: 10,
            postgres_timeout_secs: 30,
            nats_url: "nats://127.0.0.1:4222".to_owned(),
            auth_decoding_key: "./public.pem".into(),
            auth_encoding_key: "./private.pem".into(),
            openrouter_api_key: format!("sk-or-v1-{}", "A".repeat(64)),
            openrouter_base_url: Some("https://openrouter.ai/api/v1/".to_owned()),
        }
    }
}

impl From<ServiceConfig> for ServerServiceConfig {
    fn from(cli_config: ServiceConfig) -> Self {
        Self {
            postgres_endpoint: cli_config.postgres_url,
            postgres_max_connections: cli_config.postgres_max_pool,
            postgres_connection_timeout_secs: cli_config.postgres_timeout_secs,
            nats_url: cli_config.nats_url,
            auth_decoding_key: cli_config.auth_decoding_key,
            auth_encoding_key: cli_config.auth_encoding_key,
            openrouter_api_key: cli_config.openrouter_api_key,
            openrouter_base_url: cli_config.openrouter_base_url,
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
