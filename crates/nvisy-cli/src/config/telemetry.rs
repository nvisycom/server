//! Telemetry configuration management.
//!
//! This module provides configuration structures for managing telemetry settings
//! including usage analytics, crash reporting, and data collection preferences.

use clap::Args;
use serde::{Deserialize, Serialize};

/// Telemetry configuration options.
///
/// Controls all aspects of telemetry data collection including what data
/// to collect, where to send it, and how to handle the transmission.
#[derive(Debug, Clone, Serialize, Deserialize, Args)]
pub struct TelemetryConfig {
    /// Whether telemetry is enabled.
    ///
    /// This is the master switch for all telemetry functionality. When disabled,
    /// no data will be collected or transmitted regardless of other settings.
    #[arg(long, env = "NVISY_TELEMETRY_ENABLED")]
    #[serde(default)]
    pub enabled: bool,

    /// Custom endpoint for telemetry data.
    ///
    /// If not specified, the default Nvisy telemetry endpoint will be used.
    /// Must be a valid HTTP or HTTPS URL.
    #[arg(long, env = "NVISY_TELEMETRY_ENDPOINT")]
    pub endpoint: Option<String>,

    /// Timeout for telemetry requests in seconds.
    ///
    /// Controls how long to wait for telemetry requests to complete before
    /// giving up. Valid range: 1-300 seconds.
    #[arg(long, env = "NVISY_TELEMETRY_TIMEOUT", default_value_t = 10)]
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,

    /// Whether to collect usage statistics.
    ///
    /// Usage statistics include server startup/shutdown events, configuration
    /// patterns (anonymized), and feature usage patterns.
    #[arg(long, env = "NVISY_TELEMETRY_COLLECT_USAGE")]
    #[serde(default = "default_true")]
    pub collect_usage: bool,

    /// Whether to collect crash reports.
    ///
    /// Crash reports include error messages (sanitized), error codes,
    /// system information, and context data (sanitized).
    #[arg(long, env = "NVISY_TELEMETRY_COLLECT_CRASHES")]
    #[serde(default = "default_true")]
    pub collect_crashes: bool,

    /// Maximum number of telemetry events to buffer.
    ///
    /// When telemetry requests fail or are slow, events are buffered up to
    /// this limit before being dropped to prevent memory issues.
    #[arg(long, env = "NVISY_TELEMETRY_BUFFER_SIZE", default_value_t = 100)]
    #[serde(default = "default_buffer_size")]
    pub buffer_size: usize,

    /// Whether to use verbose telemetry logging.
    ///
    /// When enabled, telemetry operations will be logged at DEBUG level
    /// for troubleshooting telemetry issues.
    #[arg(long, env = "NVISY_TELEMETRY_VERBOSE")]
    #[serde(default)]
    pub verbose: bool,
}

impl TelemetryConfig {
    /// Default telemetry endpoint.
    pub const DEFAULT_ENDPOINT: &'static str = "https://api.nvisy.com/v1/telemetry/";

    /// Creates a telemetry configuration for testing.
    #[must_use]
    pub fn for_testing() -> Self {
        Self {
            enabled: true,
            endpoint: Some("http://localhost:3000/test".to_string()),
            timeout_seconds: 1,
            collect_usage: true,
            collect_crashes: true,
            buffer_size: 10,
            verbose: true,
        }
    }

    /// Returns whether the configuration appears to be for development/testing.
    #[must_use]
    pub fn is_development(&self) -> bool {
        self.endpoint.as_ref().is_some_and(|e| {
            e.contains("localhost") || e.contains("127.0.0.1") || e.contains("test")
        })
    }

    /// Validates the telemetry configuration.
    pub fn validate(&self) -> anyhow::Result<()> {
        if !self.enabled {
            return Ok(()); // No validation needed if disabled
        }

        // Validate timeout
        if !(1..=300).contains(&self.timeout_seconds) {
            return Err(anyhow::anyhow!(
                "Telemetry timeout {} seconds is invalid. Must be between 1 and 300 seconds.",
                self.timeout_seconds
            ));
        }

        // Validate endpoint if provided
        if let Some(ref endpoint) = self.endpoint
            && !endpoint.starts_with("http://")
            && !endpoint.starts_with("https://")
        {
            return Err(anyhow::anyhow!(
                "Telemetry endpoint '{endpoint}' must start with http:// or https://"
            ));
        }

        // Ensure at least one collection type is enabled
        if !self.collect_usage && !self.collect_crashes {
            return Err(anyhow::anyhow!(
                "At least one telemetry collection type (usage or crashes) must be enabled"
            ));
        }

        Ok(())
    }

    /// Gets the configured endpoint URL.
    #[must_use]
    pub fn endpoint(&self) -> &str {
        self.endpoint.as_deref().unwrap_or(Self::DEFAULT_ENDPOINT)
    }
}

impl Default for TelemetryConfig {
    /// Creates a telemetry configuration with secure defaults.
    ///
    /// Telemetry is disabled by default and must be explicitly enabled.
    fn default() -> Self {
        Self {
            enabled: false, // Opt-in by default
            endpoint: None, // Use default endpoint
            timeout_seconds: 10,
            collect_usage: true,
            collect_crashes: true,
            buffer_size: 100,
            verbose: false,
        }
    }
}

// Default value functions for serde
const fn default_timeout() -> u64 {
    10
}

const fn default_true() -> bool {
    true
}

const fn default_buffer_size() -> usize {
    100
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_disabled() {
        let config = TelemetryConfig::default();
        assert!(!config.enabled);
        assert!(config.endpoint.is_none());
        assert_eq!(config.timeout_seconds, 10);
        assert!(config.collect_usage);
        assert!(config.collect_crashes);
        assert!(!config.verbose);
    }

    #[test]
    fn validation_works_correctly() {
        let config = TelemetryConfig::default();
        assert!(config.validate().is_ok());

        let test_config = TelemetryConfig::for_testing();
        assert!(test_config.validate().is_ok());

        let mut invalid_config = TelemetryConfig::for_testing();
        invalid_config.timeout_seconds = 0;
        assert!(invalid_config.validate().is_err());

        invalid_config.timeout_seconds = 10;
        invalid_config.endpoint = Some("invalid-url".to_string());
        assert!(invalid_config.validate().is_err());

        invalid_config.endpoint = None;
        invalid_config.collect_usage = false;
        invalid_config.collect_crashes = false;
        assert!(invalid_config.validate().is_err());
    }
}
