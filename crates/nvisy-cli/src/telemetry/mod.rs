//! Telemetry and tracing configuration.

mod tracing;

use anyhow::Context;

/// Initializes the tracing subscriber based on enabled features.
///
/// # Errors
///
/// Returns an error if the tracing subscriber fails to initialize.
pub(crate) fn init_tracing() -> anyhow::Result<()> {
    #[cfg(feature = "otel")]
    {
        tracing::init_tracing_with_otel().context("Failed to initialize OpenTelemetry tracing")
    }

    #[cfg(not(feature = "otel"))]
    {
        tracing::init_tracing().context("Failed to initialize tracing")
    }
}
