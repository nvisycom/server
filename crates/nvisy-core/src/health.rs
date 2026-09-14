//! Shared health-reporting vocabulary and the [`HealthCheck`] trait.
//!
//! Each service client implements [`HealthCheck`] to report the health of the
//! component it manages as a [`ComponentHealth`]. A consumer aggregates those
//! into a [`HealthReport`]; mapping that report onto a transport (an HTTP status,
//! a response body) is left to the consumer.

use std::borrow::Cow;

use serde::Serialize;

/// Operational status of a service component.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[derive(Serialize)]
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

    /// Aggregates per-component results into an overall status: [`Unhealthy`]
    /// when there are no components or none are healthy, [`Healthy`] when all
    /// are, and [`Degraded`] otherwise.
    ///
    /// [`Unhealthy`]: Self::Unhealthy
    /// [`Healthy`]: Self::Healthy
    /// [`Degraded`]: Self::Degraded
    #[must_use]
    pub fn from_components(components: &[ComponentHealth]) -> Self {
        if components.is_empty() {
            return Self::Unhealthy;
        }

        let healthy = components.iter().filter(|c| c.status.is_healthy()).count();

        if healthy == 0 {
            Self::Unhealthy
        } else if healthy == components.len() {
            Self::Healthy
        } else {
            Self::Degraded
        }
    }
}

/// Health of a single service component.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ComponentHealth {
    /// Component name (e.g. `"postgres"`, `"nats"`).
    pub name: Cow<'static, str>,
    /// Status of this component.
    pub status: HealthStatus,
}

impl ComponentHealth {
    /// Creates a result for a healthy component.
    pub fn healthy(name: impl Into<Cow<'static, str>>) -> Self {
        Self {
            name: name.into(),
            status: HealthStatus::Healthy,
        }
    }

    /// Creates a result for an unhealthy component.
    pub fn unhealthy(name: impl Into<Cow<'static, str>>) -> Self {
        Self {
            name: name.into(),
            status: HealthStatus::Unhealthy,
        }
    }
}

/// An aggregated health report: the per-component results plus the overall
/// [`HealthStatus`] they roll up to.
///
/// A consumer probes its [`HealthCheck`]s, collects the [`ComponentHealth`]s, and
/// builds a report with [`from_components`](Self::from_components); the overall
/// status is derived once, at construction, so every reader sees the same verdict.
/// Mapping the report onto a transport (an HTTP status code, a timestamped
/// response body) is the consumer's concern.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthReport {
    /// Overall status rolled up from the components.
    pub status: HealthStatus,
    /// Per-component results.
    pub components: Vec<ComponentHealth>,
}

impl HealthReport {
    /// Builds a report from per-component results, deriving the overall status
    /// with [`HealthStatus::from_components`].
    #[must_use]
    pub fn from_components(components: Vec<ComponentHealth>) -> Self {
        Self {
            status: HealthStatus::from_components(&components),
            components,
        }
    }

    /// Whether the overall status is [`Healthy`](HealthStatus::Healthy).
    #[must_use]
    pub const fn is_healthy(&self) -> bool {
        self.status.is_healthy()
    }
}

/// Reports the health of the component a client manages.
#[async_trait::async_trait]
pub trait HealthCheck: Send + Sync {
    /// Probes the component and returns its current health.
    async fn check_health(&self) -> ComponentHealth;
}

#[cfg(test)]
mod tests {
    use super::{ComponentHealth, HealthStatus};

    #[test]
    fn no_components_is_unhealthy() {
        assert_eq!(HealthStatus::from_components(&[]), HealthStatus::Unhealthy);
    }

    #[test]
    fn all_healthy_is_healthy() {
        let components = [ComponentHealth::healthy("a"), ComponentHealth::healthy("b")];
        assert_eq!(
            HealthStatus::from_components(&components),
            HealthStatus::Healthy
        );
    }

    #[test]
    fn some_healthy_is_degraded() {
        let components = [
            ComponentHealth::healthy("a"),
            ComponentHealth::unhealthy("b"),
        ];
        assert_eq!(
            HealthStatus::from_components(&components),
            HealthStatus::Degraded
        );
    }

    #[test]
    fn none_healthy_is_unhealthy() {
        let components = [
            ComponentHealth::unhealthy("a"),
            ComponentHealth::unhealthy("b"),
        ];
        assert_eq!(
            HealthStatus::from_components(&components),
            HealthStatus::Unhealthy
        );
    }
}
