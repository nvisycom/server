//! Health monitoring service with simple caching.
//!
//! This module provides a centralized health checking system for monitoring the status
//! of all critical system components including PostgreSQL and NATS services.
//! It caches per-component results with a configurable TTL to avoid repeated expensive
//! health check operations while ensuring timely detection of service degradation.

use std::borrow::Cow;
use std::sync::Arc;
use std::time::{Duration, Instant};

use jiff::Timestamp;
use nvisy_nats::NatsClient;
use nvisy_postgres::PgClient;
use tokio::sync::RwLock;

use crate::handler::response::{ComponentCheck, Health, ServiceStatus};
use crate::service::ServiceState;

/// Tracing target for health cache operations.
const TRACING_TARGET: &str = "nvisy_server::health_cache";

/// Default cache duration for health checks.
const DEFAULT_CACHE_DURATION: Duration = Duration::from_secs(30);

/// Cached health snapshot containing per-component results and a timestamp.
#[derive(Debug, Clone)]
struct HealthSnapshot {
    /// Per-component results: `(name, healthy)`.
    components: Vec<(Cow<'static, str>, bool)>,
    /// When the snapshot was taken.
    checked_at: Instant,
    /// RFC 3339 timestamp for the response.
    timestamp: Timestamp,
}

/// Internal health cache entry.
#[derive(Debug)]
struct HealthCacheEntry {
    /// Cached health snapshot.
    snapshot: RwLock<Option<HealthSnapshot>>,
    /// How long cached values remain valid before requiring a fresh check.
    cache_duration: Duration,
}

impl HealthCacheEntry {
    fn new(cache_duration: Duration) -> Self {
        Self {
            snapshot: RwLock::new(None),
            cache_duration,
        }
    }

    /// Returns the cached snapshot if it is still valid.
    async fn get_cached(&self) -> Option<HealthSnapshot> {
        let guard = self.snapshot.read().await;
        let snapshot = guard.as_ref()?;

        if snapshot.checked_at.elapsed() < self.cache_duration {
            Some(snapshot.clone())
        } else {
            None
        }
    }

    /// Stores a new snapshot.
    async fn store(&self, snapshot: HealthSnapshot) {
        *self.snapshot.write().await = Some(snapshot);
    }

    /// Returns the last snapshot regardless of expiry.
    async fn get_last(&self) -> Option<HealthSnapshot> {
        self.snapshot.read().await.clone()
    }

    /// Forces cache invalidation.
    async fn invalidate(&self) {
        *self.snapshot.write().await = None;
    }
}

/// Health monitoring service with per-component caching.
///
/// This service provides centralized health checking for all critical system components
/// (PostgreSQL, NATS) with intelligent caching to balance responsiveness and performance.
///
/// # Thread Safety
///
/// This type is `Clone` and all clones share the same underlying cache through `Arc`.
#[derive(Debug, Clone)]
pub struct HealthCache {
    cache: Arc<HealthCacheEntry>,
}

impl HealthCache {
    /// Creates a new health monitoring service with the default cache duration of 30 seconds.
    pub fn new() -> Self {
        Self::with_cache_duration(DEFAULT_CACHE_DURATION)
    }

    /// Creates a new health monitoring service with a custom cache duration.
    pub fn with_cache_duration(cache_duration: Duration) -> Self {
        tracing::debug!(
            target: TRACING_TARGET,
            cache_duration_secs = cache_duration.as_secs(),
            "Health cache initialized"
        );

        Self {
            cache: Arc::new(HealthCacheEntry::new(cache_duration)),
        }
    }

    /// Performs a health check and returns a [`Health`] response.
    ///
    /// If the cache is still valid the cached snapshot is returned immediately.
    /// Otherwise all components are checked concurrently.
    pub async fn check(&self, service_state: &ServiceState) -> Health {
        if let Some(snapshot) = self.cache.get_cached().await {
            return Self::snapshot_to_health(snapshot);
        }

        let snapshot = self
            .check_all_components(&service_state.postgres, &service_state.nats)
            .await;
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
                status: ServiceStatus::Unhealthy,
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
        let checks: Vec<ComponentCheck> = snapshot
            .components
            .iter()
            .map(|(name, healthy)| ComponentCheck {
                name: name.clone(),
                status: if *healthy {
                    ServiceStatus::Healthy
                } else {
                    ServiceStatus::Unhealthy
                },
            })
            .collect();

        let all_healthy = snapshot.components.iter().all(|(_, h)| *h);
        let any_healthy = snapshot.components.iter().any(|(_, h)| *h);

        let status = if all_healthy {
            ServiceStatus::Healthy
        } else if any_healthy {
            ServiceStatus::Degraded
        } else {
            ServiceStatus::Unhealthy
        };

        Health {
            status,
            checks,
            timestamp: snapshot.timestamp,
        }
    }

    /// Performs concurrent health checks across all system components.
    #[tracing::instrument(skip_all, target = TRACING_TARGET)]
    async fn check_all_components(
        &self,
        pg_client: &PgClient,
        nats_client: &NatsClient,
    ) -> HealthSnapshot {
        let start = Instant::now();

        let (db_healthy, nats_healthy) =
            tokio::join!(self.check_database(pg_client), self.check_nats(nats_client));

        let check_duration = start.elapsed();
        let overall_healthy = db_healthy && nats_healthy;

        tracing::info!(
            target: TRACING_TARGET,
            duration_ms = check_duration.as_millis(),
            database_healthy = db_healthy,
            nats_healthy = nats_healthy,
            overall_healthy = overall_healthy,
            "Health check completed"
        );

        HealthSnapshot {
            components: vec![
                (Cow::Borrowed("postgres"), db_healthy),
                (Cow::Borrowed("nats"), nats_healthy),
            ],
            checked_at: start,
            timestamp: Timestamp::now(),
        }
    }

    /// Checks PostgreSQL database health by attempting to acquire a connection.
    async fn check_database(&self, pg_client: &PgClient) -> bool {
        match pg_client.get_connection().await {
            Ok(_) => {
                tracing::debug!(target: TRACING_TARGET, "Postgres health check passed");
                true
            }
            Err(e) => {
                tracing::warn!(
                    target: TRACING_TARGET,
                    error = %e,
                    "postgres health check failed"
                );
                false
            }
        }
    }

    /// Checks NATS messaging system health using a two-phase approach.
    ///
    /// 1. First checks connection state (fast, no network call)
    /// 2. Then performs a ping to verify actual connectivity (network round-trip)
    async fn check_nats(&self, nats_client: &NatsClient) -> bool {
        if !nats_client.is_connected() {
            tracing::warn!(
                target: TRACING_TARGET,
                "nats is not connected"
            );

            return false;
        }

        match nats_client.ping().await {
            Ok(duration) => {
                tracing::debug!(
                    target: TRACING_TARGET,
                    ping_ms = duration.as_millis(),
                    "nats health check passed"
                );
                true
            }
            Err(e) => {
                tracing::warn!(
                    target: TRACING_TARGET,
                    error = %e,
                    "nats health check failed"
                );
                false
            }
        }
    }
}

impl Default for HealthCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_cache_entry_creation() {
        let entry = HealthCacheEntry::new(Duration::from_secs(30));
        assert!(entry.get_cached().await.is_none());
    }

    #[tokio::test]
    async fn test_health_cache_entry_store_and_get() {
        let entry = HealthCacheEntry::new(Duration::from_secs(60));

        let snapshot = HealthSnapshot {
            components: vec![
                (Cow::Borrowed("postgres"), true),
                (Cow::Borrowed("nats"), true),
            ],
            checked_at: Instant::now(),
            timestamp: Timestamp::now(),
        };

        entry.store(snapshot).await;

        let cached = entry.get_cached().await;
        assert!(cached.is_some());

        let cached = cached.unwrap();
        assert_eq!(cached.components.len(), 2);
        assert!(cached.components.iter().all(|(_, h)| *h));
    }

    #[tokio::test]
    async fn test_health_cache_entry_expiry() {
        let entry = HealthCacheEntry::new(Duration::from_millis(10));

        let snapshot = HealthSnapshot {
            components: vec![(Cow::Borrowed("postgres"), true)],
            checked_at: Instant::now(),
            timestamp: Timestamp::now(),
        };

        entry.store(snapshot).await;
        assert!(entry.get_cached().await.is_some());

        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(entry.get_cached().await.is_none());

        // get_last still returns expired snapshot
        assert!(entry.get_last().await.is_some());
    }

    #[tokio::test]
    async fn test_health_cache_entry_invalidation() {
        let entry = HealthCacheEntry::new(Duration::from_secs(60));

        let snapshot = HealthSnapshot {
            components: vec![(Cow::Borrowed("postgres"), true)],
            checked_at: Instant::now(),
            timestamp: Timestamp::now(),
        };

        entry.store(snapshot).await;
        assert!(entry.get_cached().await.is_some());

        entry.invalidate().await;
        assert!(entry.get_cached().await.is_none());
        assert!(entry.get_last().await.is_none());
    }

    #[tokio::test]
    async fn test_snapshot_to_health_all_healthy() {
        let snapshot = HealthSnapshot {
            components: vec![
                (Cow::Borrowed("postgres"), true),
                (Cow::Borrowed("nats"), true),
            ],
            checked_at: Instant::now(),
            timestamp: Timestamp::now(),
        };

        let health = HealthCache::snapshot_to_health(snapshot);
        assert_eq!(health.status, ServiceStatus::Healthy);
        assert_eq!(health.checks.len(), 2);
    }

    #[tokio::test]
    async fn test_snapshot_to_health_degraded() {
        let snapshot = HealthSnapshot {
            components: vec![
                (Cow::Borrowed("postgres"), true),
                (Cow::Borrowed("nats"), false),
            ],
            checked_at: Instant::now(),
            timestamp: Timestamp::now(),
        };

        let health = HealthCache::snapshot_to_health(snapshot);
        assert_eq!(health.status, ServiceStatus::Degraded);
    }

    #[tokio::test]
    async fn test_snapshot_to_health_all_unhealthy() {
        let snapshot = HealthSnapshot {
            components: vec![
                (Cow::Borrowed("postgres"), false),
                (Cow::Borrowed("nats"), false),
            ],
            checked_at: Instant::now(),
            timestamp: Timestamp::now(),
        };

        let health = HealthCache::snapshot_to_health(snapshot);
        assert_eq!(health.status, ServiceStatus::Unhealthy);
    }

    #[tokio::test]
    async fn test_health_service_invalidation() {
        let service = HealthCache::new();
        service.invalidate().await;

        let health = service.get_cached_health().await;
        assert_eq!(health.status, ServiceStatus::Unhealthy);
        assert!(health.checks.is_empty());
    }

    #[test]
    fn test_health_service_creation_variants() {
        let _service1 = HealthCache::new();
        let _service2 = HealthCache::with_cache_duration(Duration::from_secs(5));
        let _service3: HealthCache = Default::default();
    }
}
