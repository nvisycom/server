//! HTTP/HTTPS server configuration and startup with comprehensive lifecycle management.
//!
//! This module provides a clean API for starting HTTP and HTTPS servers with
//! enhanced error handling and production-ready lifecycle management.
//! It automatically handles protocol selection based on TLS configuration.

/// Tracing target for server startup events.
pub const TRACING_TARGET_STARTUP: &str = "nvisy_cli::server::startup";

/// Tracing target for server shutdown events.
pub const TRACING_TARGET_SHUTDOWN: &str = "nvisy_cli::server::shutdown";

#[cfg(not(feature = "tls"))]
mod http_server;
#[cfg(feature = "tls")]
mod https_server;
mod lifecycle;
mod shutdown;

use std::io;

use axum::Router;
#[cfg(not(feature = "tls"))]
use http_server::serve_http;
#[cfg(feature = "tls")]
use https_server::serve_https;
use shutdown::shutdown_signal;

use crate::config::ServerConfig;

/// Starts a server with automatic protocol selection (HTTP/HTTPS) based on configuration.
///
/// This is a convenience function that automatically chooses between HTTP and HTTPS
/// based on whether the `tls` feature is enabled.
///
/// # Arguments
///
/// * `app` - The Axum router to serve
/// * `config` - Server configuration that determines protocol and settings
///
/// # Errors
///
/// Returns an error if:
/// - TLS certificates cannot be loaded (HTTPS mode)
/// - Cannot bind to the specified address/port
/// - Server encounters a fatal error during operation
pub async fn serve(app: Router, config: ServerConfig) -> io::Result<()> {
    #[cfg(feature = "tls")]
    {
        serve_https(app, config).await
    }

    #[cfg(not(feature = "tls"))]
    {
        serve_http(app, config).await
    }
}
