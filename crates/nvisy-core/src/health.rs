//! Health monitoring utilities for services.

use std::collections::HashMap;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Represents the operational status of a service.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServiceStatus {
    /// Service is operating normally
    #[default]
    Healthy,
    /// Service is operating with some issues but still functional
    Degraded,
    /// Service is not operational
    Unhealthy,
}

/// Health information for a service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceHealth {
    /// Current service status
    pub status: ServiceStatus,
    /// Response time in milliseconds for the health check
    pub response_time_ms: Option<u64>,
    /// Optional message describing the current state
    pub message: Option<String>,
    /// Timestamp when the health check was performed
    pub checked_at: Timestamp,
    /// Additional metrics about the service
    pub metrics: HashMap<String, Value>,
}

impl ServiceHealth {
    /// Creates a new healthy service health report.
    pub fn healthy() -> Self {
        Self {
            status: ServiceStatus::Healthy,
            response_time_ms: None,
            message: None,
            checked_at: Timestamp::now(),
            metrics: HashMap::new(),
        }
    }

    /// Creates a new degraded service health report.
    pub fn degraded(message: impl Into<String>) -> Self {
        Self {
            status: ServiceStatus::Degraded,
            response_time_ms: None,
            message: Some(message.into()),
            checked_at: Timestamp::now(),
            metrics: HashMap::new(),
        }
    }

    /// Creates a new unhealthy service health report.
    pub fn unhealthy(message: impl Into<String>) -> Self {
        Self {
            status: ServiceStatus::Unhealthy,
            response_time_ms: None,
            message: Some(message.into()),
            checked_at: Timestamp::now(),
            metrics: HashMap::new(),
        }
    }

    /// Sets the response time for this health check.
    pub fn with_response_time(mut self, ms: u64) -> Self {
        self.response_time_ms = Some(ms);
        self
    }

    /// Adds a metric to the health report.
    pub fn with_metric(mut self, key: impl Into<String>, value: Value) -> Self {
        self.metrics.insert(key.into(), value);
        self
    }
}
