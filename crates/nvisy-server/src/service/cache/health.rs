//! Health monitoring service with simple caching.
//!
//! Provides centralized health checking for all system components with
//! atomic boolean caching to avoid repeated expensive operations.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use nvisy_nats::NatsClient;
use nvisy_openrouter::LlmClient;
use nvisy_postgres::PgClient;
use tokio::sync::RwLock;

/// Tracing target for health service operations.
const TRACING_TARGET: &str = "nvisy_server::service::health";

/// Default cache duration for health checks.
const DEFAULT_CACHE_DURATION: Duration = Duration::from_secs(30);

/// Simple health cache entry with atomic boolean and timestamp.
#[derive(Debug)]
struct HealthCacheEntry {
    is_healthy: AtomicBool,
    last_check: RwLock<Instant>,
    cache_duration: Duration,
}

impl HealthCacheEntry {
    fn new(cache_duration: Duration) -> Self {
        Self {
            is_healthy: AtomicBool::new(false),
            last_check: RwLock::new(Instant::now() - cache_duration), // Force initial check
            cache_duration,
        }
    }

    /// Get cached value or update with new check.
    async fn get_or_update<F, Fut>(&self, check_fn: F) -> bool
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = bool>,
    {
        let now = Instant::now();
        let last_check = *self.last_check.read().await;

        // Check if cache is still valid
        if now.duration_since(last_check) < self.cache_duration {
            return self.is_healthy.load(Ordering::Relaxed);
        }

        // Perform health check
        let healthy = check_fn().await;

        // Update cache
        self.is_healthy.store(healthy, Ordering::Relaxed);
        *self.last_check.write().await = now;

        healthy
    }

    /// Get current cached value without updating.
    fn get_cached(&self) -> bool {
        self.is_healthy.load(Ordering::Relaxed)
    }

    /// Force invalidate the cache.
    async fn invalidate(&self) {
        *self.last_check.write().await = Instant::now() - self.cache_duration;
    }
}

/// Health monitoring service with atomic boolean caching.
///
/// Provides centralized health checking for all system components with
/// simple caching to avoid repeated expensive operations.
#[derive(Debug, Clone)]
pub struct HealthService {
    cache: Arc<HealthCacheEntry>,
}

impl HealthService {
    /// Create a new health service with default cache duration.
    pub fn new() -> Self {
        Self::with_cache_duration(DEFAULT_CACHE_DURATION)
    }

    /// Create a new health service with custom cache duration.
    pub fn with_cache_duration(cache_duration: Duration) -> Self {
        tracing::info!(
            target: TRACING_TARGET,
            cache_duration_secs = cache_duration.as_secs(),
            "Health service initialized"
        );

        Self {
            cache: Arc::new(HealthCacheEntry::new(cache_duration)),
        }
    }

    /// Check overall system health with caching.
    pub async fn is_healthy(
        &self,
        pg_client: &PgClient,
        nats_client: &NatsClient,
        llm_client: &LlmClient,
    ) -> bool {
        self.cache
            .get_or_update(|| self.check_all_components(pg_client, nats_client, llm_client))
            .await
    }

    /// Get current cached health status without updating.
    pub fn get_cached_health(&self) -> bool {
        self.cache.get_cached()
    }

    /// Invalidate cache, forcing fresh check on next access.
    pub async fn invalidate(&self) {
        self.cache.invalidate().await;

        tracing::debug!(
            target: TRACING_TARGET,
            "Health cache invalidated"
        );
    }

    /// Internal method to check all components concurrently.
    #[tracing::instrument(skip_all, target = TRACING_TARGET)]
    async fn check_all_components(
        &self,
        pg_client: &PgClient,
        nats_client: &NatsClient,
        llm_client: &LlmClient,
    ) -> bool {
        let start = Instant::now();

        // Perform all health checks concurrently
        let (db_healthy, nats_healthy, openrouter_healthy) = tokio::join!(
            self.check_database(pg_client),
            self.check_nats(nats_client),
            self.check_openrouter(llm_client)
        );

        let check_duration = start.elapsed();
        let overall_healthy = db_healthy && nats_healthy && openrouter_healthy;

        tracing::info!(
            target: TRACING_TARGET,
            duration_ms = check_duration.as_millis(),
            database_healthy = db_healthy,
            nats_healthy = nats_healthy,
            openrouter_healthy = openrouter_healthy,
            overall_healthy = overall_healthy,
            "Health check completed"
        );

        overall_healthy
    }

    /// Internal database health check.
    async fn check_database(&self, pg_client: &PgClient) -> bool {
        match pg_client.get_connection().await {
            Ok(_) => {
                tracing::debug!(target: TRACING_TARGET, "Database health check passed");
                true
            }
            Err(e) => {
                tracing::warn!(
                    target: TRACING_TARGET,
                    error = %e,
                    "Database health check failed"
                );
                false
            }
        }
    }

    /// Internal NATS health check.
    async fn check_nats(&self, nats_client: &NatsClient) -> bool {
        // First check connection state
        let stats = nats_client.stats();
        if !stats.is_connected {
            tracing::warn!(
                target: TRACING_TARGET,
                "NATS is not connected"
            );
            return false;
        }

        // Then try a ping to verify actual connectivity
        match nats_client.ping().await {
            Ok(duration) => {
                tracing::debug!(
                    target: TRACING_TARGET,
                    ping_ms = duration.as_millis(),
                    "NATS health check passed"
                );
                true
            }
            Err(e) => {
                tracing::warn!(
                    target: TRACING_TARGET,
                    error = %e,
                    "NATS health check failed"
                );
                false
            }
        }
    }

    /// Internal OpenRouter health check.
    async fn check_openrouter(&self, llm_client: &LlmClient) -> bool {
        match llm_client.list_models().await {
            Ok(_) => {
                tracing::debug!(target: TRACING_TARGET, "OpenRouter health check passed");
                true
            }
            Err(e) => {
                tracing::warn!(
                    target: TRACING_TARGET,
                    error = %e,
                    "OpenRouter health check failed"
                );
                false
            }
        }
    }
}

impl Default for HealthService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_cache_entry_creation() {
        let entry = HealthCacheEntry::new(Duration::from_secs(30));
        assert!(!entry.get_cached()); // Should start as unhealthy
    }

    #[tokio::test]
    async fn test_health_cache_entry_update() {
        let entry = HealthCacheEntry::new(Duration::from_secs(1));

        // Should perform check on first call
        let result = entry.get_or_update(|| async { true }).await;
        assert!(result);
        assert!(entry.get_cached());

        // Should return cached result on second immediate call
        let result = entry.get_or_update(|| async { false }).await;
        assert!(result); // Should still be true from cache
    }

    #[tokio::test]
    async fn test_health_cache_entry_expiry() {
        let entry = HealthCacheEntry::new(Duration::from_millis(10));

        // Set initial value
        let result = entry.get_or_update(|| async { true }).await;
        assert!(result);

        // Wait for cache to expire
        tokio::time::sleep(Duration::from_millis(20)).await;

        // Should perform new check
        let result = entry.get_or_update(|| async { false }).await;
        assert!(!result);
    }

    #[tokio::test]
    async fn test_health_cache_entry_invalidation() {
        let entry = HealthCacheEntry::new(Duration::from_secs(60)); // Long cache

        // Set initial value
        entry.get_or_update(|| async { true }).await;
        assert!(entry.get_cached());

        // Invalidate cache
        entry.invalidate().await;

        // Should perform new check even though cache duration hasn't passed
        let result = entry.get_or_update(|| async { false }).await;
        assert!(!result);
    }

    #[test]
    fn test_health_service_creation() {
        let service = HealthService::new();
        assert!(!service.get_cached_health()); // Should start as unhealthy

        let service_with_duration = HealthService::with_cache_duration(Duration::from_secs(10));
        assert!(!service_with_duration.get_cached_health());
    }

    #[test]
    fn test_health_service_default() {
        let service = HealthService::default();
        assert!(!service.get_cached_health());
    }

    #[tokio::test]
    async fn test_health_service_invalidation() {
        let service = HealthService::new();

        // This should work without panicking
        service.invalidate().await;
        assert!(!service.get_cached_health());
    }

    #[test]
    fn test_health_service_creation_variants() {
        // Test default creation
        let service1 = HealthService::new();
        assert!(!service1.get_cached_health());

        // Test with custom duration
        let service2 = HealthService::with_cache_duration(Duration::from_secs(5));
        assert!(!service2.get_cached_health());

        // Test default trait
        let service3: HealthService = Default::default();
        assert!(!service3.get_cached_health());
    }

    #[test]
    fn test_health_service_cached_health_initially_false() {
        let service = HealthService::new();

        // Cached value should start as false
        assert!(!service.get_cached_health());
        assert!(!service.cache.get_cached());
    }
}
