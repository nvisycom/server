//! CORS (Cross-Origin Resource Sharing) middleware configuration.

use std::time::Duration;

use axum::http::{HeaderValue, Method, header};
use clap::Args;
use serde::{Deserialize, Serialize};
use tower_http::cors::CorsLayer;

/// Creates a CORS layer based on the provided configuration.
///
/// # Arguments
///
/// * `config` - CORS configuration with allowed origins and settings
///
/// # Returns
///
/// A configured `CorsLayer` ready to be applied to the router
pub fn create_cors_layer(config: &CorsConfig) -> CorsLayer {
    let origins = config.to_header_values();

    CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE, header::ACCEPT])
        .expose_headers([header::AUTHORIZATION])
        .allow_credentials(config.allow_credentials)
        .max_age(config.max_age())
}

/// Creates a development CORS layer with localhost origins.
///
/// This is useful for development environments where you want
/// to allow requests from common development ports.
pub fn create_dev_cors_layer() -> CorsLayer {
    let config = CorsConfig::default();
    create_cors_layer(&config)
}

/// CORS (Cross-Origin Resource Sharing) configuration.
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

impl CorsConfig {
    /// Returns the CORS max age as a Duration.
    pub fn max_age(&self) -> Duration {
        Duration::from_secs(self.max_age_seconds)
    }

    /// Returns localhost origins for development.
    pub fn get_localhost_origins() -> Vec<HeaderValue> {
        vec![
            "http://localhost:3000".parse().unwrap(),
            "http://localhost:8080".parse().unwrap(),
            "http://127.0.0.1:3000".parse().unwrap(),
            "http://127.0.0.1:8080".parse().unwrap(),
            "http://localhost:5173".parse().unwrap(), // Vite default
        ]
    }

    /// Converts configured origins to HeaderValue list.
    pub fn to_header_values(&self) -> Vec<HeaderValue> {
        if self.allowed_origins.is_empty() {
            Self::get_localhost_origins()
        } else {
            self.allowed_origins
                .iter()
                .filter_map(|origin| origin.parse().ok())
                .collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_cors_layer() {
        let config = CorsConfig {
            allowed_origins: vec!["https://example.com".to_string()],
            max_age_seconds: 3600,
            allow_credentials: true,
        };

        let _layer = create_cors_layer(&config);
        // Layer creation should not panic
    }

    #[test]
    fn test_create_dev_cors_layer() {
        let _layer = create_dev_cors_layer();
        // Layer creation should not panic
    }

    #[test]
    fn test_cors_config_localhost_origins() {
        let config = CorsConfig::default();
        let origins = config.to_header_values();
        assert_eq!(origins.len(), 5);
    }

    #[test]
    fn test_cors_config_custom_origins() {
        let config = CorsConfig {
            allowed_origins: vec![
                "https://example.com".to_string(),
                "https://app.example.com".to_string(),
            ],
            ..Default::default()
        };
        let origins = config.to_header_values();
        assert_eq!(origins.len(), 2);
    }
}
