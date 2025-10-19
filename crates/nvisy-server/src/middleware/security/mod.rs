//! Security middleware for HTTP requests.
//!
//! This module provides middleware for:
//! - CORS (Cross-Origin Resource Sharing) configuration
//! - Security headers (HSTS, CSP, X-Frame-Options, etc.)
//! - Request body size limiting

mod body_limit;
mod cors;
mod headers_config;

pub(crate) use body_limit::create_body_limit_layer;
pub use cors::CorsConfig;
pub(crate) use cors::create_cors_layer;
pub use headers_config::{FrameOptions, ReferrerPolicy, SecurityHeadersConfig};
