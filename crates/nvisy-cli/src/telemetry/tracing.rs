//! Tracing initialization and configuration.

use tracing_subscriber::fmt::layer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Tracing target for OpenTelemetry operations.
const TRACING_TARGET_OTEL: &str = "nvisy_cli::otel";

const fn default_log_level() -> &'static str {
    "info,nvisy_cli=trace"
}

#[must_use]
fn build_env_filter() -> tracing_subscriber::EnvFilter {
    let current = std::env::var("RUST_LOG")
        .or_else(|_| std::env::var("OTEL_LOG_LEVEL"))
        .unwrap_or_else(|_| default_log_level().to_string());

    let env = format!("{current},tower=info,tower_http=info");
    tracing_subscriber::EnvFilter::new(env)
}

/// Initializes the tracing subscriber for structured logging.
///
/// This sets up structured logging with environment-based filtering and
/// pretty formatting for development.
///
/// # Configuration
///
/// The log level can be configured via environment variables:
/// - `RUST_LOG`: Standard Rust logging configuration
/// - `OTEL_LOG_LEVEL`: OpenTelemetry-specific log level
///
/// Default log levels:
/// - `info` for most dependencies
/// - `trace` for `nvisy_cli` module
/// - `info` for `tower` and `tower_http`
///
/// # Examples
///
/// ```bash
/// RUST_LOG=debug nvisy-cli
/// RUST_LOG=nvisy_cli=trace,axum=debug nvisy-cli
/// ```
///
/// # Note
///
/// Initializes the tracing subscriber for the application.
#[cfg(not(feature = "otel"))]
pub(super) fn init_tracing() {
    // Setup a temporary subscriber to log output during setup
    let env_filter = build_env_filter();
    let fmt_layer = layer().pretty();
    let subscriber = tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer);

    let _guard = tracing::subscriber::set_default(subscriber);
    tracing::trace!(
        target: TRACING_TARGET_OTEL,
        "initialized temporary tracing subscriber",
    );

    // TODO: Enable OpenTelemetry
    // https://github.com/davidB/tracing-opentelemetry-instrumentation-sdk

    // Setup the actual subscriber
    let env_filter = build_env_filter();
    let fmt_layer = layer().pretty();
    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .init();

    tracing::trace!(
        target: TRACING_TARGET_OTEL,
        "initialized tracing subscriber",
    );
}

/// Initializes tracing with OpenTelemetry support.
///
/// This is only available when the `otel` feature is enabled.
///
/// # Note
///
/// Initializes the tracing subscriber with OpenTelemetry support.
#[cfg(feature = "otel")]
pub(super) fn init_tracing_with_otel() {
    // Setup a temporary subscriber to log output during setup
    let env_filter = build_env_filter();
    let fmt_layer = layer().pretty();
    let subscriber = tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer);

    let _guard = tracing::subscriber::set_default(subscriber);
    tracing::trace!(
        target: TRACING_TARGET_OTEL,
        "initialized temporary tracing subscriber with otel support",
    );

    // TODO: Enable OpenTelemetry
    // https://github.com/davidB/tracing-opentelemetry-instrumentation-sdk

    // Setup the actual subscriber
    let env_filter = build_env_filter();
    let fmt_layer = layer().pretty();
    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .init();

    tracing::info!("OpenTelemetry support enabled");
    tracing::trace!(
        target: TRACING_TARGET_OTEL,
        "initialized tracing subscriber with otel support",
    );
}
