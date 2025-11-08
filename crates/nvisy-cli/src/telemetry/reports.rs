//! Platform telemetry for usage and crash reporting.
//!
//! This module provides functionality for collecting and reporting usage statistics
//! and crash information to help improve the software. All telemetry is optional
//! and can be disabled via configuration.
//!
//! # Privacy
//!
//! - No personally identifiable information is collected
//! - All data is anonymized with random session IDs
//! - Users can opt-out via configuration
//! - Network requests respect user privacy settings
//!
//! # Features
//!
//! This module is only available when the `telemetry` feature is enabled.

use std::collections::HashMap;
use std::env;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::TelemetryConfig;

/// Platform and system information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformInfo {
    /// Operating system name.
    pub os: String,
    /// CPU architecture.
    pub arch: String,
    /// Software version.
    pub version: String,
    /// Rust version used to compile.
    pub rust_version: String,
    /// Session ID for anonymization.
    pub session_id: String,
    /// Timestamp of data collection.
    pub timestamp: u64,
}

impl PlatformInfo {
    /// Collects current platform information.
    #[must_use]
    pub fn collect() -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            os: env::consts::OS.to_string(),
            arch: env::consts::ARCH.to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            rust_version: env!("CARGO_PKG_RUST_VERSION").to_string(),
            session_id: Uuid::new_v4().to_string(),
            timestamp,
        }
    }
}

/// Usage report data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageReport {
    /// Platform information.
    #[serde(flatten)]
    pub platform: PlatformInfo,
    /// Event type (startup, shutdown, etc.).
    pub event_type: UsageEventType,
    /// Server configuration hash (for grouping similar configs).
    pub config_hash: String,
    /// Uptime in seconds (for shutdown events).
    pub uptime_seconds: Option<u64>,
    /// Additional metadata.
    pub metadata: HashMap<String, String>,
}

/// Types of usage events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageEventType {
    /// Server startup.
    Startup,
    /// Server shutdown.
    Shutdown,
    /// Configuration change.
    ConfigChange,
    /// Feature usage.
    FeatureUsage,
}

/// Crash report data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrashReport {
    /// Platform information.
    #[serde(flatten)]
    pub platform: PlatformInfo,
    /// Error message (sanitized).
    pub error_message: String,
    /// Error code if available.
    pub error_code: Option<String>,
    /// Stack trace (if available and safe).
    pub stack_trace: Option<String>,
    /// Context information.
    pub context: HashMap<String, String>,
    /// Whether the error was recoverable.
    pub recoverable: bool,
}

/// Telemetry client for sending usage and crash reports.
pub struct TelemetryClient {
    config: TelemetryConfig,
    client: Client,
    endpoint: String,
}

impl TelemetryClient {
    /// Default telemetry endpoint.
    const DEFAULT_ENDPOINT: &'static str = "https://telemetry.nvisy.com/api/v1";

    /// Creates a new telemetry client.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be created.
    pub fn new(config: TelemetryConfig) -> Result<Self> {
        if !config.enabled {
            return Ok(Self {
                config,
                client: Client::new(),
                endpoint: String::new(),
            });
        }

        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .user_agent(concat!("nvisy-cli/", env!("CARGO_PKG_VERSION")))
            .build()
            .context("Failed to create HTTP client for telemetry")?;

        let endpoint = config
            .endpoint
            .clone()
            .unwrap_or_else(|| Self::DEFAULT_ENDPOINT.to_string());

        Ok(Self {
            config,
            client,
            endpoint,
        })
    }

    /// Sends a usage report asynchronously.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or telemetry is disabled.
    pub async fn send_usage_report(&self, mut report: UsageReport) -> Result<()> {
        if !self.config.enabled || !self.config.collect_usage {
            tracing::debug!("usage telemetry disabled, skipping report");
            return Ok(());
        }

        // Sanitize the report
        report.metadata = Self::sanitize_metadata(report.metadata);

        let url = format!("{}/usage", self.endpoint);

        tracing::debug!(
            event_type = ?report.event_type,
            session_id = %report.platform.session_id,
            "sending usage report"
        );

        let response = self
            .client
            .post(&url)
            .json(&report)
            .send()
            .await
            .context("Failed to send usage report")?;

        if response.status().is_success() {
            tracing::trace!("usage report sent successfully");
        } else {
            tracing::warn!(
                status = %response.status(),
                "usage report failed with non-success status"
            );
        }

        Ok(())
    }

    /// Sends a crash report asynchronously.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or telemetry is disabled.
    pub async fn send_crash_report(&self, mut report: CrashReport) -> Result<()> {
        if !self.config.enabled || !self.config.collect_crashes {
            tracing::debug!("crash telemetry disabled, skipping report");
            return Ok(());
        }

        // Sanitize the report
        report.error_message = Self::sanitize_error_message(report.error_message);
        report.context = Self::sanitize_metadata(report.context);
        report.stack_trace = report
            .stack_trace
            .as_deref()
            .map(Self::sanitize_stack_trace);

        let url = format!("{}/crash", self.endpoint);

        tracing::debug!(
            session_id = %report.platform.session_id,
            error_code = ?report.error_code,
            recoverable = report.recoverable,
            "sending crash report"
        );

        let response = self
            .client
            .post(&url)
            .json(&report)
            .send()
            .await
            .context("Failed to send crash report")?;

        if response.status().is_success() {
            tracing::trace!("crash report sent successfully");
        } else {
            tracing::warn!(
                status = %response.status(),
                "crash report failed with non-success status"
            );
        }

        Ok(())
    }

    /// Sends a usage report without blocking.
    pub fn send_usage_report_background(&self, report: UsageReport) {
        if !self.config.enabled || !self.config.collect_usage {
            return;
        }

        let client = self.clone();
        tokio::spawn(async move {
            if let Err(e) = client.send_usage_report(report).await {
                tracing::warn!(error = %e, "failed to send usage report in background");
            }
        });
    }

    /// Sends a crash report without blocking.
    pub fn send_crash_report_background(&self, report: CrashReport) {
        if !self.config.enabled || !self.config.collect_crashes {
            return;
        }

        let client = self.clone();
        tokio::spawn(async move {
            if let Err(e) = client.send_crash_report(report).await {
                tracing::warn!(error = %e, "failed to send crash report in background");
            }
        });
    }

    /// Sanitizes metadata to remove potentially sensitive information.
    #[must_use]
    fn sanitize_metadata(mut metadata: HashMap<String, String>) -> HashMap<String, String> {
        // Remove potentially sensitive keys
        let sensitive_keys = ["password", "token", "key", "secret", "auth", "credential"];

        metadata.retain(|key, _| {
            !sensitive_keys
                .iter()
                .any(|sensitive| key.to_lowercase().contains(sensitive))
        });

        // Truncate long values
        for value in metadata.values_mut() {
            if value.len() > 100 {
                value.truncate(97);
                value.push_str("...");
            }
        }

        metadata
    }

    /// Sanitizes error messages to remove sensitive information.
    #[must_use]
    fn sanitize_error_message(mut message: String) -> String {
        // Remove file paths that might contain usernames
        if let Some(pos) = message.find("/Users/")
            && let Some(end) = message[pos..].find(' ')
        {
            message.replace_range(pos..pos + end, "/Users/[REDACTED]");
        }

        if let Some(pos) = message.find("/home/")
            && let Some(end) = message[pos..].find(' ')
        {
            message.replace_range(pos..pos + end, "/home/[REDACTED]");
        }

        // Truncate very long messages
        if message.len() > 500 {
            message.truncate(497);
            message.push_str("...");
        }

        message
    }

    /// Sanitizes stack traces to remove sensitive information.
    #[must_use]
    fn sanitize_stack_trace(stack_trace: &str) -> String {
        // Similar sanitization as error messages
        stack_trace
            .lines()
            .take(20) // Limit stack trace depth
            .map(|line| {
                let mut line = line.to_string();
                // Remove file paths
                if line.contains("/Users/") {
                    line = line.replace("/Users/", "/Users/[REDACTED]/");
                }
                if line.contains("/home/") {
                    line = line.replace("/home/", "/home/[REDACTED]/");
                }
                line
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl Clone for TelemetryClient {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            client: self.client.clone(),
            endpoint: self.endpoint.clone(),
        }
    }
}

/// Helper functions for creating telemetry reports.
pub mod reporting {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    use super::{CrashReport, Duration, HashMap, PlatformInfo, UsageEventType, UsageReport};
    use crate::config::ServerConfig;
    use crate::server::ServerError;

    /// Creates a usage report for server startup.
    #[must_use]
    pub fn create_startup_report(server_config: &ServerConfig) -> UsageReport {
        let mut metadata = HashMap::new();
        metadata.insert("port".to_string(), server_config.port.to_string());
        metadata.insert(
            "host_type".to_string(),
            if server_config.binds_to_all_interfaces() {
                "all"
            } else {
                "local"
            }
            .to_string(),
        );

        #[cfg(feature = "tls")]
        metadata.insert(
            "tls_enabled".to_string(),
            server_config.is_tls_enabled().to_string(),
        );

        UsageReport {
            platform: PlatformInfo::collect(),
            event_type: UsageEventType::Startup,
            config_hash: hash_config(server_config),
            uptime_seconds: None,
            metadata,
        }
    }

    /// Creates a usage report for server shutdown.
    #[must_use]
    pub fn create_shutdown_report(server_config: &ServerConfig, uptime: Duration) -> UsageReport {
        let mut metadata = HashMap::new();
        metadata.insert("uptime_seconds".to_string(), uptime.as_secs().to_string());
        metadata.insert("graceful".to_string(), "true".to_string());

        UsageReport {
            platform: PlatformInfo::collect(),
            event_type: UsageEventType::Shutdown,
            config_hash: hash_config(server_config),
            uptime_seconds: Some(uptime.as_secs()),
            metadata,
        }
    }

    /// Creates a crash report from a server error.
    #[must_use]
    pub fn create_crash_report(
        error: &ServerError,
        context: HashMap<String, String>,
    ) -> CrashReport {
        CrashReport {
            platform: PlatformInfo::collect(),
            error_message: error.to_string(),
            error_code: Some(error.error_code().to_string()),
            stack_trace: None, // Could be enhanced with backtrace if needed
            context,
            recoverable: error.is_recoverable(),
        }
    }

    /// Creates a hash of the server configuration for grouping.
    #[must_use]
    fn hash_config(config: &ServerConfig) -> String {
        let mut hasher = DefaultHasher::new();

        // Hash relevant configuration fields
        config.port.hash(&mut hasher);
        config.request_timeout.hash(&mut hasher);
        config.shutdown_timeout.hash(&mut hasher);
        config.binds_to_all_interfaces().hash(&mut hasher);

        #[cfg(feature = "tls")]
        config.is_tls_enabled().hash(&mut hasher);

        format!("{:x}", hasher.finish())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn telemetry_client_disabled() {
        let config = TelemetryConfig {
            enabled: false,
            ..Default::default()
        };

        let client = TelemetryClient::new(config).unwrap();

        let report = UsageReport {
            platform: PlatformInfo::collect(),
            event_type: UsageEventType::Startup,
            config_hash: "test".to_string(),
            uptime_seconds: None,
            metadata: HashMap::new(),
        };

        // Should succeed but do nothing
        assert!(client.send_usage_report(report).await.is_ok());
    }

    #[test]
    fn sanitize_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("normal_field".to_string(), "normal_value".to_string());
        metadata.insert("password".to_string(), "secret123".to_string());
        metadata.insert("long_value".to_string(), "x".repeat(200));

        let sanitized = TelemetryClient::sanitize_metadata(metadata);

        assert!(sanitized.contains_key("normal_field"));
        assert!(!sanitized.contains_key("password"));
        assert!(
            sanitized
                .get("long_value")
                .is_some_and(|v| v.ends_with("..."))
        );
        assert!(sanitized.get("long_value").is_some_and(|v| v.len() <= 100));
    }

    #[test]
    fn sanitize_error_message() {
        let message = "Error in file /Users/username/secret/file.txt with data".to_string();
        let sanitized = TelemetryClient::sanitize_error_message(message);

        assert!(sanitized.contains("/Users/[REDACTED]"));
        assert!(!sanitized.contains("username"));
    }
}
