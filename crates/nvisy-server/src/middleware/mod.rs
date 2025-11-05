//! Middleware for `axum::Router` and HTTP request processing.
//!
//! This module provides a comprehensive set of middleware for:
//! - Authentication and authorization
//! - Security (CORS, headers, body limits)
//! - Observability (metrics, tracing, request IDs)
//! - Error handling (panics, timeouts, service errors)
//! - Rate limiting
//! - OpenAPI documentation
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use std::time::Duration;
//! use axum::Router;
//! use nvisy_server::middleware::{RouterExt, CorsConfig, SecurityHeadersConfig};
//!
//! let app = Router::new()
//!     .with_error_handling_layer(Duration::from_secs(30))
//!     .with_observability_layer()
//!     .with_default_security_layer()
//!     .with_metrics_layer();
//! ```

mod auth;
mod error_handling;
mod extensions;
mod observability;
pub mod open_api;
mod rate_limiting;
pub mod security;

pub use auth::{refresh_token_middleware, require_admin, require_authentication};
pub use extensions::RouterExt;
pub use open_api::{OpenApiConfig, RouterOpenApiExt};
pub use security::{CorsConfig, SecurityHeadersConfig};

// Tracing target constants for consistent logging.
pub const TRACING_TARGET_AUTH: &str = "nvisy_server::middleware::auth";
