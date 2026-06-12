//! Health monitoring service aggregating registered component checks.
//!
//! This module provides a centralized health checking system that aggregates
//! the [`HealthCheck`] results of all registered components. Results are cached
//! with a configurable TTL to avoid repeated expensive checks while still
//! detecting service degradation in a timely manner.

use std::sync::Arc;
use std::time::Instant;

use jiff::Timestamp;
use nvisy_core::health::{ComponentHealth, HealthCheck, HealthStatus};

use super::snapshot::{HealthCacheEntry, HealthSnapshot};
use super::{HealthConfig, TRACING_TARGET};
use crate::handler::response::Health;

/// Health monitoring service aggregating registered component checks.
///
/// The set of components to probe is registered at construction; results are
/// cached to balance responsiveness and cost.
///
/// # Thread Safety
///
/// This type is `Clone` and all clones share the same underlying cache and
/// checker set through `Arc`.
#[derive(Clone)]
pub struct HealthCache {
    cache: Arc<HealthCacheEntry>,
    checkers: Arc<[Arc<dyn HealthCheck>]>,
}

impl HealthCache {
    /// Creates a health monitor over the given components.
    pub fn new(config: &HealthConfig, checkers: Vec<Arc<dyn HealthCheck>>) -> Self {
        tracing::debug!(
            target: TRACING_TARGET,
            cache_duration = ?config.cache_duration,
            components = checkers.len(),
            "Health cache initialized"
        );

        Self {
            cache: Arc::new(HealthCacheEntry::new(config.cache_duration)),
            checkers: checkers.into(),
        }
    }

    /// Performs a health check and returns a [`Health`] response.
    ///
    /// If the cache is still valid the cached snapshot is returned immediately.
    /// Otherwise all registered components are checked concurrently.
    pub async fn check(&self) -> Health {
        if let Some(snapshot) = self.cache.get_cached().await {
            return Self::snapshot_to_health(snapshot);
        }

        let snapshot = self.check_all_components().await;
        self.cache.store(snapshot.clone()).await;
        Self::snapshot_to_health(snapshot)
    }

    /// Returns the last cached [`Health`] without performing any checks.
    ///
    /// Falls back to an unhealthy response with no component checks when
    /// no snapshot has been cached yet.
    pub async fn get_cached_health(&self) -> Health {
        match self.cache.get_last().await {
            Some(snapshot) => Self::snapshot_to_health(snapshot),
            None => Health {
                status: HealthStatus::Unhealthy,
                checks: Vec::new(),
                timestamp: Timestamp::now(),
            },
        }
    }

    /// Invalidates the health cache, forcing a fresh check on the next access.
    pub async fn invalidate(&self) {
        self.cache.invalidate().await;

        tracing::debug!(
            target: TRACING_TARGET,
            "Health cache invalidated"
        );
    }

    /// Converts a [`HealthSnapshot`] into a [`Health`] response.
    fn snapshot_to_health(snapshot: HealthSnapshot) -> Health {
        let all_healthy = snapshot.components.iter().all(|c| c.status.is_healthy());
        let any_healthy = snapshot.components.iter().any(|c| c.status.is_healthy());

        let status = if snapshot.components.is_empty() || !any_healthy {
            HealthStatus::Unhealthy
        } else if all_healthy {
            HealthStatus::Healthy
        } else {
            HealthStatus::Degraded
        };

        Health {
            status,
            checks: snapshot.components,
            timestamp: snapshot.timestamp,
        }
    }

    /// Probes all registered components concurrently.
    #[tracing::instrument(skip_all, target = TRACING_TARGET)]
    async fn check_all_components(&self) -> HealthSnapshot {
        let start = Instant::now();

        let components: Vec<ComponentHealth> =
            futures::future::join_all(self.checkers.iter().map(|c| c.check_health())).await;

        let healthy = components.iter().filter(|c| c.status.is_healthy()).count();
        tracing::info!(
            target: TRACING_TARGET,
            duration_ms = start.elapsed().as_millis(),
            healthy,
            total = components.len(),
            "Health check completed"
        );

        HealthSnapshot {
            components,
            checked_at: start,
            timestamp: Timestamp::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::*;

    /// A checker that always reports the given status.
    struct StubChecker {
        name: &'static str,
        status: HealthStatus,
    }

    #[async_trait::async_trait]
    impl HealthCheck for StubChecker {
        async fn check_health(&self) -> ComponentHealth {
            ComponentHealth {
                name: Cow::Borrowed(self.name),
                status: self.status,
                latency: None,
            }
        }
    }

    fn checker(name: &'static str, status: HealthStatus) -> Arc<dyn HealthCheck> {
        Arc::new(StubChecker { name, status })
    }

    fn cache(checkers: Vec<Arc<dyn HealthCheck>>) -> HealthCache {
        HealthCache::new(&HealthConfig::default(), checkers)
    }

    #[tokio::test]
    async fn all_healthy_is_healthy() {
        let cache = cache(vec![
            checker("a", HealthStatus::Healthy),
            checker("b", HealthStatus::Healthy),
        ]);
        let health = cache.check().await;
        assert_eq!(health.status, HealthStatus::Healthy);
        assert_eq!(health.checks.len(), 2);
    }

    #[tokio::test]
    async fn some_healthy_is_degraded() {
        let cache = cache(vec![
            checker("a", HealthStatus::Healthy),
            checker("b", HealthStatus::Unhealthy),
        ]);
        assert_eq!(cache.check().await.status, HealthStatus::Degraded);
    }

    #[tokio::test]
    async fn none_healthy_is_unhealthy() {
        let cache = cache(vec![
            checker("a", HealthStatus::Unhealthy),
            checker("b", HealthStatus::Unhealthy),
        ]);
        assert_eq!(cache.check().await.status, HealthStatus::Unhealthy);
    }

    #[tokio::test]
    async fn no_components_is_unhealthy() {
        let cache = cache(vec![]);
        let health = cache.check().await;
        assert_eq!(health.status, HealthStatus::Unhealthy);
        assert!(health.checks.is_empty());
    }
}
