//! Shared health-reporting vocabulary and the [`HealthCheck`] trait.
//!
//! Each service client implements [`HealthCheck`] to report the health of the
//! component it manages as a [`ComponentHealth`]. Aggregation into an overall
//! report (and any transport concerns) is left to the consumer.

use std::borrow::Cow;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Operational status of a service component.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    /// Component is operating normally.
    #[default]
    Healthy,
    /// Component is operating with some issues but still functional.
    Degraded,
    /// Component is not operational.
    Unhealthy,
}

impl HealthStatus {
    /// Whether the component is fully operational.
    #[must_use]
    pub const fn is_healthy(self) -> bool {
        matches!(self, Self::Healthy)
    }
}

/// Health of a single service component.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ComponentHealth {
    /// Component name (e.g. `"postgres"`, `"nats"`).
    pub name: Cow<'static, str>,
    /// Status of this component.
    pub status: HealthStatus,
    /// How long the health check took, when measured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<u64>"))]
    pub latency: Option<Duration>,
}

impl ComponentHealth {
    /// Creates a result for a healthy component.
    pub fn healthy(name: impl Into<Cow<'static, str>>) -> Self {
        Self {
            name: name.into(),
            status: HealthStatus::Healthy,
            latency: None,
        }
    }

    /// Creates a result for an unhealthy component.
    pub fn unhealthy(name: impl Into<Cow<'static, str>>) -> Self {
        Self {
            name: name.into(),
            status: HealthStatus::Unhealthy,
            latency: None,
        }
    }

    /// Attaches a measured check latency.
    #[must_use]
    pub fn with_latency(mut self, latency: Duration) -> Self {
        self.latency = Some(latency);
        self
    }
}

/// Reports the health of the component a client manages.
#[async_trait::async_trait]
pub trait HealthCheck: Send + Sync {
    /// Probes the component and returns its current health.
    async fn check_health(&self) -> ComponentHealth;
}
