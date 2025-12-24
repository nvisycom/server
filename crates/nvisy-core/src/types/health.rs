//! Health monitoring utilities for AI services.
//!
//! This module provides types for reporting and tracking service health status,
//! including operational state, response times, and custom metrics.
//!
//! Health checks are essential for monitoring service availability and performance
//! in production environments, enabling proper load balancing, circuit breaking,
//! and alerting.

use std::collections::HashMap;
use std::time::Duration;

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
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ServiceHealth {
    /// Current service status
    pub status: ServiceStatus,
    /// Response time for the health check
    pub response: Option<Duration>,
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
            checked_at: Timestamp::now(),
            ..Default::default()
        }
    }

    /// Creates a new degraded service health report.
    pub fn degraded(message: impl Into<String>) -> Self {
        Self {
            status: ServiceStatus::Degraded,
            message: Some(message.into()),
            checked_at: Timestamp::now(),
            ..Default::default()
        }
    }

    /// Creates a new unhealthy service health report.
    pub fn unhealthy(message: impl Into<String>) -> Self {
        Self {
            status: ServiceStatus::Unhealthy,
            message: Some(message.into()),
            checked_at: Timestamp::now(),
            metrics: HashMap::new(),
            ..Default::default()
        }
    }

    /// Sets the response time for this health check.
    pub fn with_response_time(mut self, response_time: Duration) -> Self {
        self.response = Some(response_time);
        self
    }

    /// Adds a metric to the health report.
    pub fn with_metric(mut self, key: impl Into<String>, value: Value) -> Self {
        self.metrics.insert(key.into(), value);
        self
    }
}
