//! Health monitoring service with simple caching.
//!
//! This module provides a centralized health checking system for monitoring the status
//! of all critical system components including PostgreSQL, NATS, and OpenRouter services.
//! It uses atomic boolean caching with configurable TTL to avoid repeated expensive
//! health check operations while ensuring timely detection of service degradation.

use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use axum::extract::FromRef;
use nvisy_nats::NatsClient;
use nvisy_openrouter::LlmClient;
use nvisy_postgres::PgClient;
use tokio::sync::RwLock;

/// Tracing target for health service operations.
const TRACING_TARGET_HEALTH: &str = "nvisy_server::service::health";

/// Default cache duration for health checks.
const DEFAULT_CACHE_DURATION: Duration = Duration::from_secs(30);

/// Internal health cache entry with atomic boolean and timestamp.
///
/// This structure stores the cached health status using an atomic boolean for
/// lock-free reads, combined with a RwLock-protected timestamp for cache expiration.
/// This design optimizes for the common case of reading cached values while still
/// allowing safe concurrent updates.
#[derive(Debug)]
struct HealthCacheEntry {
    /// Cached health status. Uses relaxed ordering since health status doesn't
    /// require strict ordering guarantees - eventual consistency is acceptable.
    is_healthy: AtomicBool,
    /// Timestamp of the last successful health check. Protected by RwLock to
    /// allow concurrent reads while preventing data races during updates.
    last_check: RwLock<Instant>,
    /// How long cached values remain valid before requiring a fresh check.
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

    /// Retrieves the cached health status or performs a fresh check if the cache has expired.
    ///
    /// This method implements a check-then-act pattern for cache validation:
    /// 1. Reads the last check timestamp (shared lock)
    /// 2. Compares against cache duration
    /// 3. Returns cached value if still valid
    /// 4. Otherwise performs health check and updates cache (exclusive lock)
    ///
    /// # Arguments
    ///
    /// * `check_fn` - Async function that performs the actual health check
    ///
    /// # Returns
    ///
    /// Current health status (either cached or freshly checked)
    async fn get_or_update<F, Fut>(&self, check_fn: F) -> bool
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = bool>,
    {
        let now = Instant::now();
        let last_check = { *self.last_check.read().await };

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

    /// Returns the current cached value without triggering any health checks.
    ///
    /// This is a lock-free operation that simply reads the atomic boolean.
    /// The value may be stale if the cache has expired but no check has been
    /// performed since expiration.
    fn get_cached(&self) -> bool {
        self.is_healthy.load(Ordering::Relaxed)
    }

    /// Forces cache invalidation by backdating the last check timestamp.
    ///
    /// After calling this method, the next call to `get_or_update` will
    /// perform a fresh health check regardless of when the last check occurred.
    /// This is useful when you know a service state has changed and want to
    /// ensure the next health check reflects current reality.
    async fn invalidate(&self) {
        *self.last_check.write().await = Instant::now() - self.cache_duration;
    }
}

/// Health monitoring service with efficient atomic boolean caching.
///
/// This service provides centralized health checking for all critical system components
/// (PostgreSQL, NATS, OpenRouter) with intelligent caching to balance responsiveness
/// and performance. Health checks are expensive operations that involve network calls,
/// so caching prevents excessive load while still detecting failures within the TTL window.
///
/// # Caching Strategy
///
/// - Health status is cached for a configurable duration (default: 30 seconds)
/// - Concurrent health checks are performed across all services
/// - Cache can be explicitly invalidated when service state changes are known
/// - Atomic operations ensure thread-safe access without locks on reads
///
/// # Thread Safety
///
/// This type is `Clone` and all clones share the same underlying cache through `Arc`.
/// All operations are thread-safe and can be called concurrently from multiple tasks.
///
/// # Example
///
/// ```no_run
/// # use nvisy_server::service::cache::HealthCache;
/// # use std::time::Duration;
/// # async fn example() {
/// let health = HealthCache::with_cache_duration(Duration::from_secs(60));
///
/// // Check health (performs actual checks or returns cached value)
/// // let is_healthy = health.is_healthy(app_state).await;
///
/// // Fast cached read without any checks
/// let cached = health.get_cached_health();
///
/// // Force next check to be fresh
/// health.invalidate().await;
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct HealthCache {
    /// Shared cache entry wrapped in Arc for cheap cloning and shared state.
    cache: Arc<HealthCacheEntry>,
}

impl HealthCache {
    /// Creates a new health monitoring service with the default cache duration of 30 seconds.
    ///
    /// This provides a good balance between responsiveness to failures and avoiding
    /// excessive health check overhead for most applications.
    pub fn new() -> Self {
        Self::with_cache_duration(DEFAULT_CACHE_DURATION)
    }

    /// Creates a new health monitoring service with a custom cache duration.
    ///
    /// # Arguments
    ///
    /// * `cache_duration` - How long health check results remain valid
    ///
    /// # Choosing a Cache Duration
    ///
    /// - **Shorter durations** (5-15s): More responsive to failures, higher overhead
    /// - **Medium durations** (30-60s): Balanced approach, suitable for most cases
    /// - **Longer durations** (2-5m): Lower overhead, slower failure detection
    ///
    /// Consider your SLA requirements and health check endpoint call frequency.
    pub fn with_cache_duration(cache_duration: Duration) -> Self {
        tracing::info!(
            target: TRACING_TARGET_HEALTH,
            cache_duration_secs = cache_duration.as_secs(),
            "health service initialized"
        );

        Self {
            cache: Arc::new(HealthCacheEntry::new(cache_duration)),
        }
    }

    /// Checks the overall system health status with intelligent caching.
    ///
    /// This method performs comprehensive health checks across all system components:
    /// - **PostgreSQL**: Verifies database connectivity
    /// - **NATS**: Checks connection state and performs ping
    /// - **OpenRouter**: Validates API connectivity
    ///
    /// All checks are performed concurrently for optimal performance. Results are
    /// cached according to the configured TTL. The system is considered healthy
    /// only if ALL components pass their health checks.
    ///
    /// # Arguments
    ///
    /// * `service_state` - Application state containing service clients
    ///
    /// # Returns
    ///
    /// `true` if all services are healthy (cached or fresh), `false` otherwise
    ///
    /// # Performance
    ///
    /// - **Cache hit**: ~nanoseconds (atomic read)
    /// - **Cache miss**: Depends on service latencies, typically 50-500ms
    pub async fn is_healthy<S>(&self, service_state: S) -> bool
    where
        PgClient: FromRef<S>,
        NatsClient: FromRef<S>,
        LlmClient: FromRef<S>,
    {
        let pg_client = PgClient::from_ref(&service_state);
        let nats_client = NatsClient::from_ref(&service_state);
        let llm_client = LlmClient::from_ref(&service_state);

        self.cache
            .get_or_update(|| self.check_all_components(&pg_client, &nats_client, &llm_client))
            .await
    }

    /// Returns the currently cached health status without performing any checks.
    ///
    /// This is an extremely fast operation (atomic load) that returns the most
    /// recently cached health status. The value may be stale if:
    /// - The cache has expired but no check has been performed since
    /// - Service states have changed since the last check
    ///
    /// Use this when you need a fast health indication and can tolerate slightly
    /// stale data (e.g., for monitoring dashboards, metrics collection).
    ///
    /// # Returns
    ///
    /// The last cached health status (`true` = healthy, `false` = unhealthy or unknown)
    pub fn get_cached_health(&self) -> bool {
        self.cache.get_cached()
    }

    /// Invalidates the health cache, forcing a fresh check on the next access.
    ///
    /// Use this method when you know service state has changed and want to ensure
    /// the next health check reflects current reality. Common scenarios:
    /// - After recovering from a known service outage
    /// - Following a service restart or deployment
    /// - When manual intervention has fixed a known issue
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use nvisy_server::service::cache::HealthCache;
    /// # async fn example(health: HealthCache) {
    /// // After restarting a failed service
    /// health.invalidate().await;
    /// // Next health check will perform fresh checks
    /// # }
    /// ```
    pub async fn invalidate(&self) {
        self.cache.invalidate().await;

        tracing::debug!(
            target: TRACING_TARGET_HEALTH,
            "Health cache invalidated"
        );
    }

    /// Performs concurrent health checks across all system components.
    ///
    /// This internal method coordinates health checking for PostgreSQL, NATS, and
    /// OpenRouter services. All checks run concurrently using `tokio::join!` to
    /// minimize total check duration.
    ///
    /// Detailed metrics are logged including per-service status and total duration.
    #[tracing::instrument(skip_all, target = TRACING_TARGET_HEALTH)]
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
            target: TRACING_TARGET_HEALTH,
            duration_ms = check_duration.as_millis(),
            database_healthy = db_healthy,
            nats_healthy = nats_healthy,
            openrouter_healthy = openrouter_healthy,
            overall_healthy = overall_healthy,
            "Health check completed"
        );

        overall_healthy
    }

    /// Checks PostgreSQL database health by attempting to acquire a connection.
    ///
    /// A successful connection acquisition indicates the database is reachable
    /// and the connection pool has available capacity.
    async fn check_database(&self, pg_client: &PgClient) -> bool {
        match pg_client.get_connection().await {
            Ok(_) => {
                tracing::debug!(target: TRACING_TARGET_HEALTH, "Postgres health check passed");
                true
            }
            Err(e) => {
                tracing::warn!(
                    target: TRACING_TARGET_HEALTH,
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
    ///
    /// This ensures both the client state and actual network connectivity are verified.
    async fn check_nats(&self, nats_client: &NatsClient) -> bool {
        // First check connection state
        let stats = nats_client.stats();
        if !stats.is_connected {
            tracing::warn!(
                target: TRACING_TARGET_HEALTH,
                "nats is not connected"
            );

            return false;
        }

        // Then try a ping to verify actual connectivity
        match nats_client.ping().await {
            Ok(duration) => {
                tracing::debug!(
                    target: TRACING_TARGET_HEALTH,
                    ping_ms = duration.as_millis(),
                    "nats health check passed"
                );
                true
            }
            Err(e) => {
                tracing::warn!(
                    target: TRACING_TARGET_HEALTH,
                    error = %e,
                    "nats health check failed"
                );
                false
            }
        }
    }

    /// Checks OpenRouter LLM service health by listing available models.
    ///
    /// A successful model list retrieval indicates the API is accessible and
    /// authentication is working correctly.
    async fn check_openrouter(&self, llm_client: &LlmClient) -> bool {
        match llm_client.list_models().await {
            Ok(_) => {
                tracing::debug!(target: TRACING_TARGET_HEALTH, "Openrouter health check passed");
                true
            }
            Err(e) => {
                tracing::warn!(
                    target: TRACING_TARGET_HEALTH,
                    error = %e,
                    "openrouter health check failed"
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

    #[tokio::test]
    async fn test_health_service_invalidation() {
        let service = HealthCache::new();

        // This should work without panicking
        service.invalidate().await;
        assert!(!service.get_cached_health());
    }

    #[test]
    fn test_health_service_creation_variants() {
        // Test default creation
        let service1 = HealthCache::new();
        assert!(!service1.get_cached_health());

        // Test with custom duration
        let service2 = HealthCache::with_cache_duration(Duration::from_secs(5));
        assert!(!service2.get_cached_health());

        // Test default trait
        let service3: HealthCache = Default::default();
        assert!(!service3.get_cached_health());
    }

    #[test]
    fn test_health_service_cached_health_initially_false() {
        let service = HealthCache::new();

        // Cached value should start as false
        assert!(!service.get_cached_health());
        assert!(!service.cache.get_cached());
    }
}
