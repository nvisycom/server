//! The health service: probes registered components and caches the result.

use std::sync::Arc;
use std::time::{Duration, Instant};

use jiff::Timestamp;
use nvisy_core::health::{ComponentHealth, HealthCheck, HealthReport};
use tokio::sync::RwLock;

use super::{HealthConfig, TRACING_TARGET};

/// A health probe result: the aggregated [`HealthReport`] and when it was taken.
///
/// The service hands this to callers; a caller maps it onto its transport (an
/// HTTP status from the report's status, a response body carrying the timestamp).
#[derive(Debug, Clone)]
pub struct HealthReading {
    /// The aggregated component report.
    pub report: HealthReport,
    /// RFC 3339 wall-clock time the probe ran.
    pub timestamp: Timestamp,
}

/// Aggregates the [`HealthCheck`]s of the registered components, caching the
/// result with a TTL so repeated probes stay cheap.
///
/// Cloneable: every clone shares one cache and one checker set through `Arc`.
///
/// [`HealthCheck`]: nvisy_core::health::HealthCheck
#[derive(Clone)]
#[must_use = "service does nothing unless you use it"]
pub struct HealthService {
    checkers: Arc<[Arc<dyn HealthCheck>]>,
    cache: Arc<Cache>,
}

impl HealthService {
    /// Creates a service over the given components, caching results for
    /// [`cache_duration`](HealthConfig::cache_duration).
    pub fn new(config: &HealthConfig, checkers: Vec<Arc<dyn HealthCheck>>) -> Self {
        tracing::debug!(
            target: TRACING_TARGET,
            cache_duration = ?config.cache_duration,
            components = checkers.len(),
            "Health service initialized",
        );

        Self {
            checkers: checkers.into(),
            cache: Arc::new(Cache::new(config.cache_duration)),
        }
    }

    /// Returns a reading no older than the TTL: the cached one while it is still
    /// valid, otherwise a fresh probe of every component. The TTL is the whole
    /// freshness contract, so this one method serves every caller — a frequent
    /// uptime probe reuses the cache, and the first refresh after it expires pays
    /// for the probe.
    pub async fn report(&self) -> HealthReading {
        if let Some(reading) = self.cache.fresh().await {
            return reading;
        }
        self.probe().await
    }

    /// Probes every component concurrently, caches the reading, and returns it.
    #[tracing::instrument(skip_all, target = TRACING_TARGET)]
    async fn probe(&self) -> HealthReading {
        let start = Instant::now();
        let components: Vec<ComponentHealth> =
            futures::future::join_all(self.checkers.iter().map(|c| c.check_health())).await;

        let report = HealthReport::from_components(components);
        tracing::info!(
            target: TRACING_TARGET,
            duration_ms = start.elapsed().as_millis(),
            status = ?report.status,
            total = report.components.len(),
            "Health probe completed",
        );

        let reading = HealthReading {
            report,
            timestamp: Timestamp::now(),
        };
        self.cache.store(start, reading.clone()).await;
        reading
    }
}

/// A cached reading behind a TTL, with the monotonic instant the probe ran so
/// staleness is measured against the clock, not the wall time in the reading.
struct Cache {
    slot: RwLock<Option<(Instant, HealthReading)>>,
    ttl: Duration,
}

impl Cache {
    fn new(ttl: Duration) -> Self {
        Self {
            slot: RwLock::new(None),
            ttl,
        }
    }

    /// The cached reading if it is still within the TTL.
    async fn fresh(&self) -> Option<HealthReading> {
        let guard = self.slot.read().await;
        let (checked_at, reading) = guard.as_ref()?;
        (checked_at.elapsed() < self.ttl).then(|| reading.clone())
    }

    /// Replaces the cached reading, stamped with when its probe ran.
    async fn store(&self, checked_at: Instant, reading: HealthReading) {
        *self.slot.write().await = Some((checked_at, reading));
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use nvisy_core::health::HealthStatus;

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
            }
        }
    }

    fn checker(name: &'static str, status: HealthStatus) -> Arc<dyn HealthCheck> {
        Arc::new(StubChecker { name, status })
    }

    fn service(checkers: Vec<Arc<dyn HealthCheck>>) -> HealthService {
        HealthService::new(&HealthConfig::default(), checkers)
    }

    #[tokio::test]
    async fn all_healthy_is_healthy() {
        let service = service(vec![
            checker("a", HealthStatus::Healthy),
            checker("b", HealthStatus::Healthy),
        ]);
        let reading = service.report().await;
        assert_eq!(reading.report.status, HealthStatus::Healthy);
        assert_eq!(reading.report.components.len(), 2);
    }

    #[tokio::test]
    async fn some_healthy_is_degraded() {
        let service = service(vec![
            checker("a", HealthStatus::Healthy),
            checker("b", HealthStatus::Unhealthy),
        ]);
        assert_eq!(service.report().await.report.status, HealthStatus::Degraded);
    }

    #[tokio::test]
    async fn none_healthy_is_unhealthy() {
        let service = service(vec![
            checker("a", HealthStatus::Unhealthy),
            checker("b", HealthStatus::Unhealthy),
        ]);
        assert_eq!(
            service.report().await.report.status,
            HealthStatus::Unhealthy
        );
    }

    #[tokio::test]
    async fn no_components_is_unhealthy() {
        let service = service(vec![]);
        let reading = service.report().await;
        assert_eq!(reading.report.status, HealthStatus::Unhealthy);
        assert!(reading.report.components.is_empty());
    }

    #[tokio::test]
    async fn cold_cache_probes_immediately() {
        // The first call, with nothing cached, must probe and reflect the real
        // component status rather than a spurious empty/unhealthy response.
        let service = service(vec![checker("a", HealthStatus::Healthy)]);
        let reading = service.report().await;
        assert_eq!(reading.report.status, HealthStatus::Healthy);
        assert_eq!(reading.report.components.len(), 1);
    }

    #[tokio::test]
    async fn second_call_is_served_from_cache() {
        // A stub that flips to unhealthy after its first probe: if the second
        // `report` re-probed, it would observe the flip; the cached value must not.
        #[derive(Default)]
        struct FlipChecker {
            probed: std::sync::atomic::AtomicBool,
        }
        #[async_trait::async_trait]
        impl HealthCheck for FlipChecker {
            async fn check_health(&self) -> ComponentHealth {
                use std::sync::atomic::Ordering;
                let status = if self.probed.swap(true, Ordering::SeqCst) {
                    HealthStatus::Unhealthy
                } else {
                    HealthStatus::Healthy
                };
                ComponentHealth {
                    name: Cow::Borrowed("flip"),
                    status,
                }
            }
        }

        let service = service(vec![Arc::new(FlipChecker::default())]);
        assert_eq!(service.report().await.report.status, HealthStatus::Healthy);
        // Within the (default 30s) TTL, the second call reuses the first reading.
        assert_eq!(service.report().await.report.status, HealthStatus::Healthy);
    }
}
