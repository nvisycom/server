//! Graceful shutdown signal handling.

use std::time::{Duration, Instant};

use tokio::signal::ctrl_c;
#[cfg(unix)]
use tokio::signal::unix;

use crate::TRACING_TARGET_SHUTDOWN;

/// Waits for a shutdown signal and performs cleanup.
///
/// This function listens for:
/// - SIGTERM (Unix/Linux)
/// - SIGINT (Ctrl+C on all platforms)
///
/// When a signal is received, it initiates graceful shutdown and ensures all
/// resources are properly cleaned up within the specified timeout period.
///
/// # Arguments
///
/// * `shutdown_timeout` - Maximum duration to wait for cleanup operations
///
/// # Behavior
///
/// - On Unix systems: Listens for both SIGTERM and SIGINT
/// - On other systems: Listens for Ctrl+C only
/// - Logs tracing information about the shutdown process
/// - Handles OpenTelemetry shutdown when the `otel` feature is enabled
pub async fn shutdown_signal(shutdown_timeout: Duration) {
    let ctrl_c = async {
        ctrl_c().await.expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        unix::signal(unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    let t0 = Instant::now();

    tracing::trace!(
        target: TRACING_TARGET_SHUTDOWN,
        timeout = shutdown_timeout.as_millis(),
        "global tracer provider is closing"
    );

    #[cfg(feature = "otel")]
    let (tx, rx) = std::sync::mpsc::channel();
    #[cfg(feature = "otel")]
    let _ = std::thread::spawn(move || {
        // TODO: Setup opentelemetry.
        // opentelemetry::global::shutdown_tracer_provider();
        tx.send(()).ok()
    });

    #[cfg(feature = "otel")]
    if rx.recv_timeout(shutdown_timeout).is_err() {
        tracing::error!(
            target: TRACING_TARGET_SHUTDOWN,
            timeout = shutdown_timeout.as_millis(),
            "global tracer provider failed to close"
        );
    }

    let t1 = Instant::now().duration_since(t0);
    tracing::warn!(
        target: TRACING_TARGET_SHUTDOWN,
        timeout = shutdown_timeout.as_millis(),
        waiting = t1.as_millis(),
        "server is terminating",
    );
}
