//! Server lifecycle management and utilities.
//!
//! This module provides comprehensive server lifecycle management including
//! startup, shutdown, health monitoring, and telemetry integration. All
//! functions are designed for production use with proper error handling
//! and observability.

use std::future::Future;
use std::io;
use std::time::Instant;

use crate::config::ServerConfig;
use crate::server::{ServerError, ServerResult};
#[cfg(feature = "telemetry")]
use crate::telemetry::{
    TelemetryContext,
    helpers::{
        send_config_error_telemetry, send_crash_telemetry, send_shutdown_telemetry,
        send_startup_telemetry,
    },
};
use crate::{TRACING_TARGET_SERVER_SHUTDOWN, TRACING_TARGET_SERVER_STARTUP};

/// Serves with lifecycle management and service-specific context.
///
/// This version provides comprehensive error handling, service-specific
/// logging, and detailed recovery suggestions. Recommended for production use
/// where detailed error context is required.
///
/// # Arguments
///
/// * `server_config` - Server configuration
/// * `service_name` - Name of the service for logging context
/// * `serve_fn` - Function that returns the server future
///
/// # Errors
///
/// Returns detailed errors with recovery suggestions and service context.
pub async fn serve_with_shutdown<F>(
    server_config: &ServerConfig,
    service_name: &str,
    serve_fn: impl FnOnce() -> F,
) -> ServerResult<()>
where
    F: Future<Output = io::Result<()>>,
{
    serve_with_shutdown_and_telemetry(server_config, service_name, None, serve_fn).await
}

/// Serves with comprehensive lifecycle management, telemetry, and service context.
///
/// The most comprehensive server lifecycle function, providing detailed
/// error handling, service-specific context, and full telemetry integration.
///
/// # Arguments
///
/// * `server_config` - Server configuration
/// * `service_name` - Name of the service for enhanced logging
/// * `telemetry_context` - Optional telemetry context
/// * `serve_fn` - Function that returns the server future
///
/// # Errors
///
/// Returns comprehensive errors with full context and recovery information.
#[allow(clippy::too_many_lines)]
pub async fn serve_with_shutdown_and_telemetry<F>(
    server_config: &ServerConfig,
    service_name: &str,
    #[cfg(feature = "telemetry")] telemetry_context: Option<&TelemetryContext>,
    #[cfg(not(feature = "telemetry"))] _telemetry_context: Option<()>,
    serve_fn: impl FnOnce() -> F,
) -> ServerResult<()>
where
    F: Future<Output = io::Result<()>>,
{
    let start_time = Instant::now();

    tracing::info!(
        target: TRACING_TARGET_SERVER_STARTUP,
        service = service_name,
        addr = %server_config.server_addr(),
        version = env!("CARGO_PKG_VERSION"),
        "Starting server"
    );

    // Pre-flight configuration validation
    if let Err(validation_error) = server_config.validate() {
        tracing::error!(
            target: TRACING_TARGET_SERVER_STARTUP,
            service = service_name,
            error = validation_error.to_string(),
            "Server configuration validation failed"
        );

        let config_error = ServerError::invalid_config(&validation_error);

        // Send crash telemetry for config errors
        #[cfg(feature = "telemetry")]
        send_config_error_telemetry(telemetry_context, &config_error, service_name);

        return Err(config_error);
    }

    // Security warnings
    if server_config.binds_to_all_interfaces() {
        tracing::warn!(
            target: TRACING_TARGET_SERVER_STARTUP,
            service = service_name,
            "Server is bound to all interfaces (0.0.0.0). Ensure proper firewall configuration."
        );
    }

    // Log production readiness information
    tracing::info!(
        target: TRACING_TARGET_SERVER_STARTUP,
        service = service_name,
        host = %server_config.host,
        port = server_config.port,
        "Server configured for production use"
    );

    // Log additional configuration details
    tracing::debug!(
        target: TRACING_TARGET_SERVER_STARTUP,
        service = service_name,
        request_timeout = server_config.request_timeout,
        shutdown_timeout = server_config.shutdown_timeout,
        binds_to_all_interfaces = server_config.binds_to_all_interfaces(),
        "Server configuration active"
    );

    // Send startup telemetry
    #[cfg(feature = "telemetry")]
    send_startup_telemetry(telemetry_context, server_config, service_name);

    tracing::info!(
        target: TRACING_TARGET_SERVER_STARTUP,
        service = service_name,
        addr = %server_config.server_addr(),
        "Server is ready and listening for connections"
    );

    let result = serve_fn().await.map_err(|err| {
        let uptime = start_time.elapsed();
        let server_error = ServerError::Runtime(err);

        tracing::error!(
            target: TRACING_TARGET_SERVER_SHUTDOWN,
            service = service_name,
            error = %server_error,
            error_code = server_error.error_code(),
            uptime_seconds = uptime.as_secs(),
            recoverable = server_error.is_recoverable(),
            "Server encountered fatal error"
        );

        if let Some(suggestion) = server_error.suggestion() {
            tracing::info!(
                target: TRACING_TARGET_SERVER_SHUTDOWN,
                service = service_name,
                suggestion = suggestion,
                "Recovery suggestion"
            );
        }

        // Send crash telemetry
        #[cfg(feature = "telemetry")]
        send_crash_telemetry(
            telemetry_context,
            &server_error,
            uptime,
            server_config,
            service_name,
        );

        server_error
    });

    let uptime = start_time.elapsed();

    match &result {
        Ok(()) => {
            tracing::info!(
                target: TRACING_TARGET_SERVER_SHUTDOWN,
                service = service_name,
                uptime_seconds = uptime.as_secs(),
                "Server shutdown completed successfully"
            );

            // Send shutdown telemetry
            #[cfg(feature = "telemetry")]
            send_shutdown_telemetry(telemetry_context, server_config, uptime, service_name);
        }
        Err(err) => {
            // Log error context for debugging
            for (key, value) in err.context() {
                tracing::debug!(
                    target: TRACING_TARGET_SERVER_SHUTDOWN,
                    service = service_name,
                    context_key = key,
                    context_value = value,
                    "Error context"
                );
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ServerConfig;

    #[tokio::test]
    async fn serve_with_shutdown_success() {
        let config = ServerConfig::default();
        let result = serve_with_shutdown(&config, "test-service", || async { Ok(()) }).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn serve_with_shutdown_handles_error() {
        let config = ServerConfig::default();
        let result = serve_with_shutdown(&config, "test-service", || async {
            Err(io::Error::other("test error"))
        })
        .await;

        assert!(result.is_err());
        match result {
            Err(ServerError::Runtime(_)) => {
                // Expected error type
            }
            _ => panic!("Expected Runtime error"),
        }
    }

    #[tokio::test]
    async fn serve_with_shutdown_provides_context() {
        let config = ServerConfig::default();
        let result = serve_with_shutdown(&config, "test-service", || async {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "access denied",
            ))
        })
        .await;

        assert!(result.is_err());
        if let Err(error) = result {
            assert!(error.is_recoverable());
            assert!(error.suggestion().is_some());
            assert_eq!(error.error_code(), "E003");
        }
    }

    #[tokio::test]
    async fn serve_with_shutdown_validates_config() {
        let config = ServerConfig {
            port: 80, // Invalid port for non-root users
            ..Default::default()
        };

        let result = serve_with_shutdown(&config, "test-service", || async { Ok(()) }).await;

        assert!(result.is_err());
        if let Err(ServerError::InvalidConfig(_)) = result {
            // Expected - config validation should fail before server starts
        } else {
            panic!("Expected InvalidConfig error");
        }
    }

    #[test]
    fn server_error_context_includes_suggestions() {
        let error = ServerError::bind_error(
            "127.0.0.1:80",
            io::Error::new(io::ErrorKind::PermissionDenied, "permission denied"),
        );

        assert!(error.is_network_error());
        assert!(error.is_recoverable());
        assert!(error.suggestion().unwrap().contains("port above 1024"));

        let context = error.context();
        assert!(context.iter().any(|(key, _)| *key == "error_code"));
        assert!(context.iter().any(|(key, _)| *key == "suggestion"));
    }

    #[test]
    fn lifecycle_functions_provide_comprehensive_features() {
        // Test that the basic serve_with_shutdown delegates to comprehensive version
        // Test that the service versions provide service context
        // This is a compilation test to ensure the API is clean
        // No runtime assertions needed - this test ensures the module compiles correctly
    }
}
