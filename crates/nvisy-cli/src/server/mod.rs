//! HTTP server configuration and startup.

mod error;
mod http_server;
#[cfg(feature = "tls")]
mod https_server;
mod shutdown;

use axum::Router;
pub use error::{Result, ServerError};
pub use http_server::serve_http;
#[cfg(feature = "tls")]
pub use https_server::serve_https;
pub(crate) use shutdown::shutdown_signal;

use crate::config::ServerConfig;

/// Starts the server with the appropriate protocol (HTTP or HTTPS).
///
/// This function automatically determines whether to start an HTTP or HTTPS server
/// based on the TLS configuration:
/// - If TLS certificate and key paths are provided (when `tls` feature is enabled),
///   starts an HTTPS server
/// - Otherwise, starts a regular HTTP server
///
/// # Arguments
///
/// * `app` - The Axum router to serve
/// * `config` - Server configuration
///
/// # Errors
///
/// Returns an error if:
/// - Server configuration is invalid
/// - TLS certificates cannot be loaded (when TLS is configured)
/// - Cannot bind to the specified address/port
/// - Server encounters a fatal error during operation
///
/// # Examples
///
/// ```no_run
/// use axum::Router;
/// use nvisy_cli::config::ServerConfig;
/// use nvisy_cli::server::serve;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let app = Router::new();
/// let config = ServerConfig::default();
///
/// serve(app, config).await?;
/// # Ok(())
/// # }
/// ```
pub async fn serve(app: Router, config: ServerConfig) -> Result<()> {
    #[cfg(feature = "tls")]
    {
        // Extract TLS paths before consuming config to avoid borrow checker issues
        if let (Some(cert_path), Some(key_path)) =
            (config.tls_cert_path.clone(), config.tls_key_path.clone())
        {
            tracing::info!("Starting HTTPS server with TLS");
            return serve_https(app, config, &cert_path, &key_path).await;
        }
    }

    tracing::info!("Starting HTTP server");
    serve_http(app, config).await
}
