//! Tracing initialization and configuration.

use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt};

/// Initializes the tracing subscriber for structured logging.
///
/// # Configuration
///
/// The log level can be configured via the `RUST_LOG` environment variable.
/// If not set, defaults to `info` level.
///
/// # Examples
///
/// ```bash
/// RUST_LOG=debug nvisy-cli
/// RUST_LOG=nvisy_cli=trace,axum=debug nvisy-cli
/// ```
///
/// # Errors
///
/// Returns an error if the tracing subscriber fails to initialize.
#[cfg(not(feature = "otel"))]
pub(super) fn init_tracing() -> anyhow::Result<()> {
    let env_filter = create_env_filter()?;
    let fmt_layer = create_fmt_layer();

    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(env_filter)
        .try_init()
        .map_err(|e| anyhow::anyhow!("Failed to initialize tracing: {e}"))?;

    Ok(())
}

/// Initializes tracing with OpenTelemetry support.
///
/// This is only available when the `otel` feature is enabled.
///
/// # Errors
///
/// Returns an error if the tracing subscriber fails to initialize.
#[cfg(feature = "otel")]
pub(super) fn init_tracing_with_otel() -> anyhow::Result<()> {
    let env_filter = create_env_filter()?;
    let fmt_layer = create_fmt_layer();

    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(env_filter)
        .try_init()
        .map_err(|e| anyhow::anyhow!("Failed to initialize tracing: {e}"))?;

    tracing::info!("OpenTelemetry support enabled");
    Ok(())
}

/// Creates an environment filter for tracing.
fn create_env_filter() -> anyhow::Result<EnvFilter> {
    EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .map_err(|e| anyhow::anyhow!("Failed to create env filter: {e}"))
}

/// Creates a formatted tracing layer.
fn create_fmt_layer() -> fmt::Layer<tracing_subscriber::Registry> {
    fmt::layer()
        .with_target(true)
        .with_thread_ids(false)
        .with_level(true)
        .with_ansi(true)
}
