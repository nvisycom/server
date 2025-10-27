//! Telemetry and tracing configuration.

#[cfg(feature = "telemetry")]
pub mod context;

#[cfg(feature = "telemetry")]
pub mod helpers;
#[cfg(feature = "telemetry")]
pub mod reports;
mod tracing;

use anyhow::Context;
#[cfg(feature = "telemetry")]
pub use context::TelemetryContext;
#[cfg(feature = "telemetry")]
pub use reports::{TelemetryClient, reporting};

/// Initializes the tracing subscriber based on enabled features.
///
/// # Errors
///
/// Returns an error if the tracing subscriber fails to initialize.
pub fn init_tracing() -> anyhow::Result<()> {
    #[cfg(feature = "otel")]
    {
        tracing::init_tracing_with_otel().context("Failed to initialize OpenTelemetry tracing")
    }

    #[cfg(not(feature = "otel"))]
    {
        tracing::init_tracing().context("Failed to initialize tracing")
    }
}
