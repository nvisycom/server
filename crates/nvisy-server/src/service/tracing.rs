//! Observability and tracing configuration.

use tracing_subscriber::fmt::layer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Tracing target for OpenTelemetry operations.
const TRACING_TARGET_OTEL: &str = "nvisy_server::otel";

const fn default_log_level() -> &'static str {
    "info,server=trace,database=trace"
}

#[must_use]
fn build_env_filter() -> tracing_subscriber::EnvFilter {
    let current = std::env::var("RUST_LOG")
        .or_else(|_| std::env::var("OTEL_LOG_LEVEL"))
        .unwrap_or_else(|_| default_log_level().to_string());

    let env = format!("{current},tower=info,tower_http=info");
    tracing_subscriber::EnvFilter::new(env)
}

/// Initializes the tracing subscriber for the application.
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
/// - `trace` for `server` and `database` modules
/// - `info` for `tower` and `tower_http`
///
/// # Example
///
/// ```rust,no_run
/// use nvisy_server::service::initialize_tracing;
///
/// fn main() -> anyhow::Result<()> {
///     initialize_tracing()?;
///     // Application code...
///     Ok(())
/// }
/// ```
///
/// # Errors
///
/// Returns an error if the subscriber cannot be initialized.
pub fn initialize_tracing() -> anyhow::Result<()> {
    // Setups a temporary subscriber to log output during setup.
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

    // TODO: Enable OpenTelemetry.
    // https://github.com/davidB/tracing-opentelemetry-instrumentation-sdk

    // Setups an actual subscriber.
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

    Ok(())
}
