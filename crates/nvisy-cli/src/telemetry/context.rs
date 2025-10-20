//! Telemetry runtime context and state management.
//!
//! This module provides runtime context for telemetry operations, wrapping
//! configuration with session state and providing utilities for determining
//! when telemetry should be collected.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use uuid::Uuid;

use crate::config::TelemetryConfig;

/// Runtime telemetry context.
///
/// Wraps telemetry configuration with runtime state including session tracking
/// and explicit enablement validation. This context is used throughout the
/// application to determine when telemetry should be collected.
#[derive(Debug, Clone)]
pub struct TelemetryContext {
    /// The telemetry configuration.
    pub config: Arc<TelemetryConfig>,
    /// Session identifier for this application instance.
    pub session_id: String,
    /// Whether telemetry was explicitly enabled by the user.
    pub explicitly_enabled: bool,
}

impl TelemetryContext {
    /// Creates a new telemetry context from configuration.
    #[must_use]
    pub fn new(config: TelemetryConfig, explicitly_enabled: bool) -> Self {
        Self {
            config: Arc::new(config),
            session_id: Uuid::new_v4().to_string(),
            explicitly_enabled,
        }
    }

    /// Returns whether telemetry is enabled and properly configured.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.config.enabled && self.explicitly_enabled
    }

    /// Returns whether usage statistics should be collected.
    #[must_use]
    pub fn should_collect_usage(&self) -> bool {
        self.is_active() && self.config.collect_usage
    }

    /// Returns whether crash reports should be collected.
    #[must_use]
    pub fn should_collect_crashes(&self) -> bool {
        self.is_active() && self.config.collect_crashes
    }

    /// Gets the configured endpoint URL.
    #[must_use]
    pub fn endpoint(&self) -> &str {
        self.config.endpoint()
    }

    /// Gets the request timeout as a Duration.
    #[must_use]
    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.config.timeout_seconds)
    }

    /// Returns whether verbose telemetry logging is enabled.
    #[must_use]
    pub fn is_verbose(&self) -> bool {
        self.config.verbose
    }

    /// Returns the buffer size for telemetry events.
    #[must_use]
    pub fn buffer_size(&self) -> usize {
        self.config.buffer_size
    }

    /// Validates the telemetry context.
    ///
    /// This validates both the underlying configuration and the runtime state.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is invalid or the context state
    /// is inconsistent.
    pub fn validate(&self) -> Result<()> {
        // Validate the underlying configuration
        self.config.validate()?;

        // Validate session ID format (should be a valid UUID)
        if self.session_id.is_empty() {
            return Err(anyhow::anyhow!("Session ID cannot be empty"));
        }

        // Validate UUID format
        if Uuid::parse_str(&self.session_id).is_err() {
            return Err(anyhow::anyhow!("Session ID is not a valid UUID"));
        }

        Ok(())
    }

    /// Returns whether the configuration appears to be for development/testing.
    #[must_use]
    pub fn is_development(&self) -> bool {
        self.config.is_development()
    }

    /// Creates a disabled telemetry context.
    #[must_use]
    pub fn disabled() -> Self {
        Self::new(TelemetryConfig::default(), false)
    }

    /// Creates a context for testing purposes.
    #[must_use]
    pub fn for_testing() -> Self {
        Self::new(TelemetryConfig::for_testing(), true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_respects_explicit_enablement() {
        let config = TelemetryConfig::for_testing();

        let context_explicit = TelemetryContext::new(config.clone(), true);
        assert!(context_explicit.is_active());
        assert!(context_explicit.should_collect_usage());
        assert!(context_explicit.should_collect_crashes());

        let context_implicit = TelemetryContext::new(config, false);
        assert!(!context_implicit.is_active());
        assert!(!context_implicit.should_collect_usage());
        assert!(!context_implicit.should_collect_crashes());
    }

    #[test]
    fn disabled_context_never_collects() {
        let context = TelemetryContext::disabled();
        assert!(!context.is_active());
        assert!(!context.should_collect_usage());
        assert!(!context.should_collect_crashes());
    }

    #[test]
    fn context_validation_works() {
        let context = TelemetryContext::for_testing();
        assert!(context.validate().is_ok());

        let disabled_context = TelemetryContext::disabled();
        assert!(disabled_context.validate().is_ok());
    }

    #[test]
    fn context_has_valid_session_id() {
        let context = TelemetryContext::for_testing();
        assert!(!context.session_id.is_empty());
        assert!(Uuid::parse_str(&context.session_id).is_ok());
    }

    #[test]
    fn context_provides_config_access() {
        let context = TelemetryContext::for_testing();
        assert!(!context.endpoint().is_empty());
        assert!(context.timeout().as_secs() > 0);
        assert!(context.buffer_size() > 0);
    }

    #[test]
    fn development_detection_works() {
        let test_context = TelemetryContext::for_testing();
        assert!(test_context.is_development());

        let prod_config = TelemetryConfig {
            enabled: true,
            endpoint: Some("https://telemetry.nvisy.com/api/v1".to_string()),
            ..TelemetryConfig::default()
        };
        let prod_context = TelemetryContext::new(prod_config, true);
        assert!(!prod_context.is_development());
    }
}
