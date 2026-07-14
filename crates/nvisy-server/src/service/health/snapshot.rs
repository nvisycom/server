//! TTL-cached storage primitive for health snapshots.

use std::time::{Duration, Instant};

use jiff::Timestamp;
use nvisy_core::health::ComponentHealth;
use tokio::sync::RwLock;

/// Cached health snapshot containing per-component results and a timestamp.
#[derive(Debug, Clone)]
pub(super) struct HealthSnapshot {
    /// Per-component health results.
    pub(super) components: Vec<ComponentHealth>,
    /// When the snapshot was taken.
    pub(super) checked_at: Instant,
    /// RFC 3339 timestamp for the response.
    pub(super) timestamp: Timestamp,
}

/// A health snapshot behind a TTL: cached values expire after `cache_duration`.
#[derive(Debug)]
pub(super) struct HealthCacheEntry {
    /// Cached health snapshot.
    snapshot: RwLock<Option<HealthSnapshot>>,
    /// How long cached values remain valid before requiring a fresh check.
    cache_duration: Duration,
}

impl HealthCacheEntry {
    pub(super) fn new(cache_duration: Duration) -> Self {
        Self {
            snapshot: RwLock::new(None),
            cache_duration,
        }
    }

    /// Returns the cached snapshot if it is still valid.
    pub(super) async fn get_cached(&self) -> Option<HealthSnapshot> {
        let guard = self.snapshot.read().await;
        let snapshot = guard.as_ref()?;

        if snapshot.checked_at.elapsed() < self.cache_duration {
            Some(snapshot.clone())
        } else {
            None
        }
    }

    /// Stores a new snapshot.
    pub(super) async fn store(&self, snapshot: HealthSnapshot) {
        *self.snapshot.write().await = Some(snapshot);
    }

    /// Returns the last snapshot regardless of expiry.
    pub(super) async fn get_last(&self) -> Option<HealthSnapshot> {
        self.snapshot.read().await.clone()
    }

    /// Forces cache invalidation.
    pub(super) async fn invalidate(&self) {
        *self.snapshot.write().await = None;
    }
}
