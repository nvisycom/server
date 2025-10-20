//! Telemetry helper functions for reporting server events.

use std::collections::HashMap;
use std::time::Duration;

#[cfg(feature = "telemetry")]
use super::{TelemetryClient, TelemetryContext, reporting};
use crate::TRACING_TARGET_TELEMETRY;
use crate::config::ServerConfig;
use crate::server::ServerError;

/// Sends configuration error telemetry.
#[cfg(feature = "telemetry")]
pub fn send_config_error_telemetry(
    telemetry_context: Option<&TelemetryContext>,
    config_error: &ServerError,
    service_name: &str,
) {
    if let Some(context) = telemetry_context
        && context.should_collect_crashes()
    {
        match TelemetryClient::new((*context.config).clone()) {
            Ok(client) => {
                let mut crash_context = HashMap::with_capacity(2);
                crash_context.insert("service".to_string(), service_name.to_string());
                crash_context.insert("phase".to_string(), "validation".to_string());

                let crash_report = reporting::create_crash_report(config_error, crash_context);
                client.send_crash_report_background(crash_report);
            }
            Err(e) => {
                tracing::warn!(
                    target: TRACING_TARGET_TELEMETRY,
                    error = %e,
                    "Failed to create telemetry client for config error report"
                );
            }
        }
    }
}

/// Sends startup telemetry with service context.
#[cfg(feature = "telemetry")]
pub fn send_startup_telemetry(
    telemetry_context: Option<&TelemetryContext>,
    server_config: &ServerConfig,
    service_name: &str,
) {
    if let Some(context) = telemetry_context
        && context.should_collect_usage()
    {
        match TelemetryClient::new((*context.config).clone()) {
            Ok(client) => {
                let mut startup_report = reporting::create_startup_report(server_config);
                startup_report
                    .metadata
                    .insert("service".to_string(), service_name.to_string());
                client.send_usage_report_background(startup_report);
            }
            Err(e) => {
                tracing::warn!(
                    target: TRACING_TARGET_TELEMETRY,
                    error = %e,
                    "Failed to create telemetry client for startup report"
                );
            }
        }
    }
}

/// Sends crash telemetry with service context.
#[cfg(feature = "telemetry")]
pub fn send_crash_telemetry(
    telemetry_context: Option<&TelemetryContext>,
    server_error: &ServerError,
    uptime: Duration,
    server_config: &ServerConfig,
    service_name: &str,
) {
    if let Some(context) = telemetry_context
        && context.should_collect_crashes()
    {
        match TelemetryClient::new((*context.config).clone()) {
            Ok(client) => {
                let mut crash_context = HashMap::with_capacity(3);
                crash_context.insert("service".to_string(), service_name.to_string());
                crash_context.insert("uptime_seconds".to_string(), uptime.as_secs().to_string());
                crash_context.insert(
                    "server_addr".to_string(),
                    server_config.server_addr().to_string(),
                );

                let crash_report = reporting::create_crash_report(server_error, crash_context);
                client.send_crash_report_background(crash_report);
            }
            Err(e) => {
                tracing::warn!(
                    target: TRACING_TARGET_TELEMETRY,
                    error = %e,
                    "Failed to create telemetry client for crash report"
                );
            }
        }
    }
}

/// Sends shutdown telemetry with service context.
#[cfg(feature = "telemetry")]
pub fn send_shutdown_telemetry(
    telemetry_context: Option<&TelemetryContext>,
    server_config: &ServerConfig,
    uptime: Duration,
    service_name: &str,
) {
    if let Some(context) = telemetry_context
        && context.should_collect_usage()
    {
        match TelemetryClient::new((*context.config).clone()) {
            Ok(client) => {
                let mut shutdown_report = reporting::create_shutdown_report(server_config, uptime);
                shutdown_report
                    .metadata
                    .insert("service".to_string(), service_name.to_string());
                client.send_usage_report_background(shutdown_report);
            }
            Err(e) => {
                tracing::warn!(
                    target: TRACING_TARGET_TELEMETRY,
                    error = %e,
                    "Failed to create telemetry client for shutdown report"
                );
            }
        }
    }
}
