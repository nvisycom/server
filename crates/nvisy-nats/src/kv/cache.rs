//! Generic caching using NATS KV.

use std::time::Duration;

use async_nats::jetstream;
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};

use super::KvStore;
use crate::{Result, TRACING_TARGET_KV};

/// Generic cache store
pub struct CacheStore {
    store: KvStore,
}

impl CacheStore {
    /// Create a new cache store
    #[instrument(skip(jetstream), target = TRACING_TARGET_KV)]
    pub async fn new(
        jetstream: &jetstream::Context,
        namespace: &str,
        ttl: Option<Duration>,
    ) -> Result<Self> {
        let bucket_name = format!("cache_{}", namespace);
        let description = format!("Cache for {}", namespace);

        let store = KvStore::new(jetstream, &bucket_name, Some(&description), ttl).await?;

        debug!(
            target: TRACING_TARGET_KV,
            namespace = %namespace,
            bucket = %bucket_name,
            ttl_secs = ttl.map(|d| d.as_secs()),
            "Created cache store"
        );

        Ok(Self { store })
    }

    /// Set a cached value
    #[instrument(skip(self, value), target = TRACING_TARGET_KV)]
    pub async fn set<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        self.store.put(key, value).await?;
        debug!(
            target: TRACING_TARGET_KV,
            key = %key,
            "Cached value"
        );
        Ok(())
    }

    /// Get a cached value
    #[instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn get<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Result<Option<T>> {
        let result = self.store.get(key).await?;
        debug!(
            target: TRACING_TARGET_KV,
            key = %key,
            cache_hit = result.is_some(),
            "Retrieved cached value"
        );
        Ok(result)
    }

    /// Delete a cached value
    #[instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn delete(&self, key: &str) -> Result<()> {
        self.store.delete(key).await?;
        debug!(
            target: TRACING_TARGET_KV,
            key = %key,
            "Deleted cached value"
        );
        Ok(())
    }

    /// Check if a key exists in cache
    #[instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn exists(&self, key: &str) -> Result<bool> {
        self.store.exists(key).await
    }

    /// Get or compute a value (cache-aside pattern)
    #[instrument(skip(self, compute_fn), target = TRACING_TARGET_KV)]
    pub async fn get_or_compute<T, F, Fut>(&self, key: &str, compute_fn: F) -> Result<T>
    where
        T: Serialize + for<'de> Deserialize<'de>,
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        // Try to get from cache first
        if let Some(cached) = self.get::<T>(key).await? {
            debug!(
                target: TRACING_TARGET_KV,
                key = %key,
                "Cache hit, returning cached value"
            );
            return Ok(cached);
        }

        // Cache miss, compute the value
        debug!(
            target: TRACING_TARGET_KV,
            key = %key,
            "Cache miss, computing value"
        );
        let value = compute_fn().await?;

        // Store in cache
        self.set(key, &value).await?;

        Ok(value)
    }

    /// Invalidate all cache entries
    #[instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn clear(&self) -> Result<()> {
        self.store.purge_all().await?;
        debug!(
            target: TRACING_TARGET_KV,
            bucket = %self.store.bucket_name(),
            "Cleared all cache entries"
        );
        Ok(())
    }

    /// Get cache statistics
    pub async fn stats(&self) -> Result<CacheStats> {
        let keys = self.store.keys().await?;

        Ok(CacheStats {
            entry_count: keys.len(),
            bucket_name: self.store.bucket_name().to_string(),
        })
    }

    /// Get the underlying store reference
    pub fn inner(&self) -> &KvStore {
        &self.store
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub entry_count: usize,
    pub bucket_name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestData {
        id: u64,
        name: String,
    }

    // Note: These tests would require a running NATS server with JetStream enabled

    #[test]
    #[ignore]
    fn test_cache_operations() {
        // Would test set/get/delete operations
    }

    #[test]
    #[ignore]
    fn test_get_or_compute() {
        // Would test cache-aside pattern
    }
}
