//! HTTPS server startup with TLS support.

use std::net::SocketAddr;
use std::path::Path;

use axum::Router;
use axum_server::tls_rustls::RustlsConfig;

use crate::config::ServerConfig;
use crate::server::http_server::serve_with_shutdown;
use crate::server::{Result, ServerError, shutdown_signal};

/// Starts an HTTPS server with TLS support and graceful shutdown.
///
/// This function validates the configuration, loads TLS certificates, binds to
/// the specified address, and starts serving requests over HTTPS with support
/// for graceful shutdown.
///
/// # Arguments
///
/// * `app` - The Axum router to serve
/// * `server_config` - Server configuration including host, port, and timeouts
/// * `cert_path` - Path to the TLS certificate file (PEM format)
/// * `key_path` - Path to the TLS private key file (PEM format)
///
/// # Errors
///
/// Returns an error if:
/// - Server configuration is invalid
/// - TLS certificates cannot be loaded or are invalid
/// - Cannot bind to the specified address/port
/// - Server encounters a fatal error during operation
///
/// # Examples
///
/// ```no_run
/// use axum::Router;
/// use nvisy_cli::config::ServerConfig;
/// use nvisy_cli::server::serve_https;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let app = Router::new();
/// let config = ServerConfig::default();
///
/// serve_https(app, config, "/path/to/cert.pem", "/path/to/key.pem").await?;
/// # Ok(())
/// # }
/// ```
pub async fn serve_https(
    app: Router,
    server_config: ServerConfig,
    cert_path: impl AsRef<Path>,
    key_path: impl AsRef<Path>,
) -> Result<()> {
    // Validate configuration before starting
    if let Err(validation_error) = server_config.validate() {
        tracing::error!(
            target: "server::startup",
            error = validation_error.to_string(),
            "Invalid server configuration"
        );

        return Err(ServerError::InvalidConfig(validation_error.to_string()));
    }

    // Log server configuration
    tracing::info!(
        target: "server::startup",
        host = server_config.host.to_string(),
        port = server_config.port,
        request_timeout_sec = server_config.request_timeout,
        shutdown_timeout_sec = server_config.shutdown_timeout,
        development_mode = server_config.is_development(),
        binds_to_all_interfaces = server_config.binds_to_all_interfaces(),
        tls = true,
        "Server configuration loaded"
    );

    let server_addr = server_config.server_addr();

    // Load TLS configuration
    let tls_config = match RustlsConfig::from_pem_file(&cert_path, &key_path).await {
        Ok(config) => {
            tracing::info!(
                target: "server::startup",
                cert_path = cert_path.as_ref().display().to_string(),
                key_path = key_path.as_ref().display().to_string(),
                "TLS certificates loaded successfully"
            );
            config
        }
        Err(tls_err) => {
            tracing::error!(
                target: "server::startup",
                cert_path = cert_path.as_ref().display().to_string(),
                key_path = key_path.as_ref().display().to_string(),
                error = tls_err.to_string(),
                "Failed to load TLS certificates"
            );

            return Err(ServerError::TlsCertificate(format!(
                "Failed to load TLS certificates: {}",
                tls_err
            )));
        }
    };

    // Start TLS server
    let handle = axum_server::Handle::new();
    let shutdown_handle = handle.clone();
    let shutdown_timeout = server_config.shutdown_timeout();

    // Spawn shutdown signal handler
    tokio::spawn(async move {
        shutdown_signal(shutdown_timeout).await;
        shutdown_handle.graceful_shutdown(Some(shutdown_timeout));
    });

    serve_with_shutdown(&server_config, || async move {
        axum_server::bind_rustls(server_addr, tls_config)
            .handle(handle)
            .serve(app.into_make_service_with_connect_info::<SocketAddr>())
            .await
    })
    .await
}
