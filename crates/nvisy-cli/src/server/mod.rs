//! HTTP/HTTPS server configuration and startup with comprehensive lifecycle management.
//!
//! This module provides a clean API for starting HTTP and HTTPS servers with optional
//! telemetry support, enhanced error handling, and production-ready lifecycle management.
//! It automatically handles protocol selection based on TLS configuration.

mod error;
mod http_server;
#[cfg(feature = "tls")]
mod https_server;
mod lifecycle;
mod shutdown;

use axum::Router;
pub use error::{ServerError, ServerResult};
use http_server::serve_http;
#[cfg(feature = "telemetry")]
use http_server::serve_http_with_telemetry;
#[cfg(feature = "tls")]
use https_server::serve_https;
#[cfg(all(feature = "tls", feature = "telemetry"))]
use https_server::serve_https_with_telemetry;
use lifecycle::serve_with_shutdown;
#[cfg(feature = "telemetry")]
pub use lifecycle::serve_with_shutdown_and_telemetry;
use shutdown::shutdown_signal;

use crate::config::ServerConfig;
#[cfg(feature = "telemetry")]
use crate::telemetry::TelemetryContext;

/// Starts a server with automatic protocol selection (HTTP/HTTPS) based on configuration.
///
/// This is a convenience function that automatically chooses between HTTP and HTTPS
/// based on whether TLS certificate paths are configured in the `ServerConfig`.
///
/// # Arguments
///
/// * `app` - The Axum router to serve
/// * `config` - Server configuration that determines protocol and settings
///
/// # Errors
///
/// Returns an error if:
/// - Server configuration is invalid
/// - TLS certificates are configured but cannot be loaded (HTTPS mode)
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
/// let config = ServerConfig::default(); // Will use HTTP
///
/// serve(app, config).await?;
/// # Ok(())
/// # }
/// ```
pub async fn serve(app: Router, config: ServerConfig) -> ServerResult<()> {
    #[cfg(feature = "tls")]
    {
        let tls_enabled = config.is_tls_enabled();
        if tls_enabled {
            let cert_path = config.tls_cert_path.as_ref().ok_or_else(|| {
                ServerError::InvalidConfig("TLS enabled but no cert path provided".to_string())
            })?;
            let key_path = config.tls_key_path.as_ref().ok_or_else(|| {
                ServerError::InvalidConfig("TLS enabled but no key path provided".to_string())
            })?;

            let cert_path = cert_path.clone();
            let key_path = key_path.clone();
            return serve_https(app, config, cert_path.as_path(), key_path.as_path()).await;
        }
        serve_http(app, config).await
    }

    #[cfg(not(feature = "tls"))]
    {
        let tls_enabled = config.is_tls_enabled();
        if tls_enabled {
            return Err(ServerError::InvalidConfig(
                "TLS is configured but the 'tls' feature is not enabled".to_string(),
            ));
        }
        serve_http(app, config).await
    }
}

/// Starts a server with automatic protocol selection and telemetry support.
///
/// This enhanced version includes telemetry integration when the telemetry
/// feature is enabled, providing usage analytics and crash reporting.
///
/// # Arguments
///
/// * `app` - The Axum router to serve
/// * `config` - Server configuration that determines protocol and settings
/// * `telemetry_context` - Optional telemetry context for reporting
///
/// # Errors
///
/// Returns the same errors as `serve`. Telemetry failures are logged
/// but do not cause server startup to fail.
#[cfg(feature = "telemetry")]
pub async fn serve_with_telemetry(
    app: Router,
    config: ServerConfig,
    telemetry_context: Option<&TelemetryContext>,
) -> ServerResult<()> {
    #[cfg(feature = "tls")]
    {
        if !config.is_tls_enabled() {
            return serve_http_with_telemetry(app, config, telemetry_context).await;
        }

        let cert_path = config.tls_cert_path.as_ref().ok_or_else(|| {
            ServerError::InvalidConfig("TLS enabled but no cert path provided".to_string())
        })?;
        let key_path = config.tls_key_path.as_ref().ok_or_else(|| {
            ServerError::InvalidConfig("TLS enabled but no key path provided".to_string())
        })?;

        let cert_path = cert_path.clone();
        let key_path = key_path.clone();
        return serve_https_with_telemetry(
            app,
            config,
            cert_path.as_path(),
            key_path.as_path(),
            telemetry_context,
        )
        .await;
    }

    #[cfg(not(feature = "tls"))]
    {
        let tls_enabled = config.is_tls_enabled();
        if tls_enabled {
            return Err(ServerError::InvalidConfig(
                "TLS is configured but the 'tls' feature is not enabled".to_string(),
            ));
        }
        return serve_http_with_telemetry(app, config, telemetry_context).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_types_provide_comprehensive_context() {
        let bind_err = ServerError::bind_error(
            "127.0.0.1:80",
            std::io::Error::new(std::io::ErrorKind::PermissionDenied, "permission denied"),
        );

        // Test error code
        assert_eq!(bind_err.error_code(), "E002");

        // Test recoverability
        assert!(bind_err.is_recoverable());

        // Test suggestions
        assert!(bind_err.suggestion().is_some());

        // Test classification
        assert!(bind_err.is_network_error());

        let context = bind_err.context();
        assert!(!context.is_empty());
        assert!(context.iter().any(|(key, _)| *key == "error_code"));
        assert!(context.iter().any(|(key, _)| *key == "suggestion"));
    }

    #[test]
    fn server_error_categories_are_comprehensive() {
        let config_err = ServerError::InvalidConfig("test".to_string());
        let bind_err = ServerError::bind_error("127.0.0.1:80", std::io::Error::other("test"));
        let runtime_err = ServerError::Runtime(std::io::Error::other("test"));
        let tls_err = ServerError::TlsCertificate("test".to_string());

        // Test error codes
        assert_eq!(config_err.error_code(), "E001");
        assert_eq!(bind_err.error_code(), "E002");
        assert_eq!(runtime_err.error_code(), "E003");
        assert_eq!(tls_err.error_code(), "E004");
    }
}
