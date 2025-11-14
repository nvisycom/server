//! Telemetry and tracing configuration.

#[cfg(feature = "telemetry")]
pub mod context;

#[cfg(feature = "telemetry")]
pub mod helpers;
#[cfg(feature = "telemetry")]
pub mod reports;
mod tracing;

#[cfg(feature = "telemetry")]
pub use context::TelemetryContext;
#[cfg(feature = "telemetry")]
pub use reports::{TelemetryClient, reporting};

/// Initializes the tracing subscriber based on enabled features.
///
/// Initializes tracing with OpenTelemetry support.
#[cfg(feature = "otel")]
pub fn init_tracing() {
    tracing::init_tracing_with_otel();
}

#[cfg(not(feature = "otel"))]
pub fn init_tracing() {
    tracing::init_tracing();
}
