//! Type-safe generic caching using NATS KV store.

use std::marker::PhantomData;
use std::time::Duration;

use async_nats::jetstream;
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::KvStore;
use crate::{Result, TRACING_TARGET_KV};

/// Type-safe generic cache store wrapper around KvStore.
///
/// Provides cache-specific semantics and operations while maintaining
/// compile-time type safety for cached values of type T.
#[derive(Clone)]
pub struct CacheStore<T> {
    store: KvStore<T>,
    namespace: String,
    _marker: PhantomData<T>,
}

impl<T> CacheStore<T>
where
    T: Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
{
    /// Create a new type-safe cache store for the given namespace.
    ///
    /// # Arguments
    /// * `jetstream` - JetStream context for NATS operations
    /// * `namespace` - Cache namespace (becomes part of bucket name)
    /// * `ttl` - Optional time-to-live for cache entries
    #[tracing::instrument(skip(jetstream), target = TRACING_TARGET_KV)]
    pub async fn new(
        jetstream: &jetstream::Context,
        namespace: &str,
        ttl: Option<Duration>,
    ) -> Result<Self> {
        let bucket_name = format!("cache_{}", namespace);
        let description = format!("Type-safe cache for {}", namespace);

        let store = KvStore::new(jetstream, &bucket_name, Some(&description), ttl).await?;

        tracing::debug!(
            target: TRACING_TARGET_KV,
            namespace = %namespace,
            bucket = %bucket_name,
            ttl_secs = ttl.map(|d| d.as_secs()),
            type_name = std::any::type_name::<T>(),
            "Created type-safe cache store"
        );

        Ok(Self {
            store,
            namespace: namespace.to_string(),
            _marker: PhantomData,
        })
    }

    /// Set a value in the cache.
    #[tracing::instrument(skip(self, value), target = TRACING_TARGET_KV)]
    pub async fn set(&self, key: &str, value: &T) -> Result<()> {
        self.store.set(key, value).await?;
        tracing::debug!(
            target: TRACING_TARGET_KV,
            key = %key,
            namespace = %self.namespace,
            "Cached value"
        );
        Ok(())
    }

    /// Get a value from the cache.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn get(&self, key: &str) -> Result<Option<T>> {
        let result = self.store.get_value(key).await?;
        tracing::debug!(
            target: TRACING_TARGET_KV,
            key = %key,
            namespace = %self.namespace,
            cache_hit = result.is_some(),
            "Retrieved cached value"
        );
        Ok(result)
    }

    /// Delete a value from the cache.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn delete(&self, key: &str) -> Result<()> {
        self.store.delete(key).await?;
        tracing::debug!(
            target: TRACING_TARGET_KV,
            key = %key,
            namespace = %self.namespace,
            "Deleted cached value"
        );
        Ok(())
    }

    /// Check if a key exists in the cache.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn exists(&self, key: &str) -> Result<bool> {
        self.store.exists(key).await
    }

    /// Get or compute a value using the cache-aside pattern.
    ///
    /// If the key exists in cache, returns the cached value.
    /// If not, computes the value using the provided function,
    /// stores it in cache, and returns it.
    #[tracing::instrument(skip(self, compute_fn), target = TRACING_TARGET_KV)]
    pub async fn get_or_compute<F, Fut>(&self, key: &str, compute_fn: F) -> Result<T>
    where
        F: FnOnce() -> Fut + Send,
        Fut: std::future::Future<Output = Result<T>> + Send,
    {
        // Check cache first
        if let Some(cached) = self.get(key).await? {
            tracing::debug!(
                target: TRACING_TARGET_KV,
                key = %key,
                namespace = %self.namespace,
                "Cache hit"
            );
            return Ok(cached);
        }

        tracing::debug!(
            target: TRACING_TARGET_KV,
            key = %key,
            namespace = %self.namespace,
            "Cache miss, computing value"
        );

        // Compute new value
        let value = compute_fn().await?;

        // Store in cache
        self.set(key, &value).await?;

        tracing::debug!(
            target: TRACING_TARGET_KV,
            key = %key,
            namespace = %self.namespace,
            "Computed and cached new value"
        );

        Ok(value)
    }

    /// Set multiple values in the cache as a batch operation.
    #[tracing::instrument(skip(self, items), target = TRACING_TARGET_KV)]
    pub async fn set_batch(&self, items: &[(&str, &T)]) -> Result<()> {
        self.store.put_batch(items).await?;
        tracing::debug!(
            target: TRACING_TARGET_KV,
            count = items.len(),
            namespace = %self.namespace,
            "Batch cached values"
        );
        Ok(())
    }

    /// Get multiple values from the cache as a batch operation.
    #[tracing::instrument(skip(self, keys), target = TRACING_TARGET_KV)]
    pub async fn get_batch(&self, keys: &[&str]) -> Result<Vec<Option<T>>> {
        let kv_results = self.store.get_batch(keys).await?;
        let mut results = Vec::with_capacity(keys.len());

        for key in keys {
            if let Some(kv_value) = kv_results.get(*key) {
                results.push(Some(kv_value.value.clone()));
            } else {
                results.push(None);
            }
        }

        let hit_count = results.iter().filter(|r| r.is_some()).count();
        tracing::debug!(
            target: TRACING_TARGET_KV,
            requested = keys.len(),
            found = hit_count,
            hit_rate = format!("{:.1}%", (hit_count as f64 / keys.len() as f64) * 100.0),
            namespace = %self.namespace,
            "Batch retrieved cached values"
        );

        Ok(results)
    }

    /// Clear all entries from the cache.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn clear(&self) -> Result<()> {
        self.store.purge_all().await?;
        tracing::info!(
            target: TRACING_TARGET_KV,
            namespace = %self.namespace,
            bucket = %self.store.bucket_name(),
            "Cleared all cache entries"
        );
        Ok(())
    }

    /// Get all keys currently in the cache.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn keys(&self) -> Result<Vec<String>> {
        self.store.keys().await
    }

    /// Get cache statistics.
    pub async fn stats(&self) -> Result<CacheStats> {
        let keys = self.store.keys().await?;

        let stats = CacheStats {
            entry_count: keys.len(),
            bucket_name: self.store.bucket_name().to_string(),
            namespace: self.namespace.clone(),
            type_name: std::any::type_name::<T>().to_string(),
        };

        tracing::debug!(
            target: TRACING_TARGET_KV,
            namespace = %self.namespace,
            entry_count = stats.entry_count,
            type_name = %stats.type_name,
            "Retrieved cache statistics"
        );

        Ok(stats)
    }

    /// Get the cache namespace.
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// Get the underlying KV store.
    pub fn inner(&self) -> &KvStore<T> {
        &self.store
    }

    /// Get the bucket name used by this cache.
    pub fn bucket_name(&self) -> &str {
        self.store.bucket_name()
    }
}

/// Cache statistics and metadata.
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Number of entries currently in cache
    pub entry_count: usize,
    /// NATS KV bucket name
    pub bucket_name: String,
    /// Cache namespace
    pub namespace: String,
    /// Rust type name of cached values
    pub type_name: String,
}

impl CacheStats {
    /// Check if cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entry_count == 0
    }

    /// Get a human-readable summary of cache stats.
    pub fn summary(&self) -> String {
        format!(
            "Cache '{}' contains {} {} entries in bucket '{}'",
            self.namespace, self.entry_count, self.type_name, self.bucket_name
        )
    }
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[allow(dead_code)]
    struct TestData {
        id: u64,
        name: String,
    }

    #[test]
    fn test_cache_stats() {
        let stats = CacheStats {
            entry_count: 5,
            bucket_name: "cache_test".to_string(),
            namespace: "test".to_string(),
            type_name: "TestData".to_string(),
        };

        assert!(!stats.is_empty());
        assert!(stats.summary().contains("5 TestData entries"));

        let empty_stats = CacheStats {
            entry_count: 0,
            bucket_name: "cache_empty".to_string(),
            namespace: "empty".to_string(),
            type_name: "TestData".to_string(),
        };

        assert!(empty_stats.is_empty());
        assert!(empty_stats.summary().contains("0 TestData entries"));
    }

    #[test]
    fn test_cache_namespace_formatting() {
        // Test that namespace is correctly formatted into bucket name
        let namespace = "user_sessions";
        let expected_bucket = "cache_user_sessions";
        let actual_bucket = format!("cache_{}", namespace);
        assert_eq!(actual_bucket, expected_bucket);
    }

    // Note: Integration tests requiring NATS server would go in a separate test module
    // or be marked with #[ignore] attribute for optional execution
}
