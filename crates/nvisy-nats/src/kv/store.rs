//! Type-safe NATS KV store wrapper with improved API design.

use std::collections::HashMap;
use std::marker::PhantomData;
use std::time::Duration;

use async_nats::jetstream::{self, kv};
use futures::StreamExt;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::{Error, Result, TRACING_TARGET_KV};

/// Type-safe NATS KV store wrapper with improved API design
///
/// This store provides a generic interface over NATS KV for a specific
/// serializable data type T, with consistent error handling and
/// comprehensive operations. The type parameter ensures compile-time
/// type safety for all operations.
#[derive(Clone)]
pub struct KvStore<T> {
    store: kv::Store,
    bucket_name: String,
    _marker: PhantomData<T>,
}

impl<T> KvStore<T>
where
    T: Serialize + DeserializeOwned + Send + Sync + 'static,
{
    /// Create or get a KV bucket for the specified type T.
    ///
    /// # Arguments
    /// * `jetstream` - JetStream context for NATS operations
    /// * `bucket_name` - Name of the KV bucket to create or access
    /// * `description` - Optional description for the bucket
    /// * `ttl` - Optional time-to-live for entries in the bucket
    #[tracing::instrument(skip(jetstream), target = TRACING_TARGET_KV)]
    pub async fn new(
        jetstream: &jetstream::Context,
        bucket_name: &str,
        description: Option<&str>,
        ttl: Option<Duration>,
    ) -> Result<Self> {
        let mut config = kv::Config {
            bucket: bucket_name.to_string(),
            description: description.unwrap_or("").to_string(),
            max_age: ttl.unwrap_or(Duration::from_secs(0)),
            ..Default::default()
        };

        if let Some(ttl_duration) = ttl {
            config.max_age = ttl_duration;
        }

        // Try to get existing bucket first
        let store = match jetstream.get_key_value(bucket_name).await {
            Ok(store) => {
                tracing::debug!(
                    target: TRACING_TARGET_KV,
                    bucket = %bucket_name,
                    "Using existing KV bucket"
                );
                store
            }
            Err(_) => {
                // Bucket doesn't exist, create it
                tracing::debug!(
                    target: TRACING_TARGET_KV,
                    bucket = %bucket_name,
                    ttl_secs = ttl.map(|d| d.as_secs()),
                    "Creating new KV bucket"
                );
                jetstream
                    .create_key_value(config)
                    .await
                    .map_err(|e| Error::operation("kv_create", e.to_string()))?
            }
        };

        Ok(Self {
            store,
            bucket_name: bucket_name.to_string(),
            _marker: PhantomData,
        })
    }

    /// Get the bucket name
    pub fn bucket_name(&self) -> &str {
        &self.bucket_name
    }

    /// Put a value into the store (serializes to JSON).
    #[tracing::instrument(skip(self, value), target = TRACING_TARGET_KV)]
    pub async fn put(&self, key: &str, value: &T) -> Result<KvEntry> {
        let json = serde_json::to_vec(value)?;
        let size = json.len();
        let revision = self
            .store
            .put(key, json.into())
            .await
            .map_err(|e| Error::operation("kv_put", e.to_string()))?;

        tracing::debug!(
            target: TRACING_TARGET_KV,
            key = %key,
            revision = revision,
            size_bytes = size,
            "Put value to KV store"
        );

        Ok(KvEntry {
            key: key.to_string(),
            revision,
            size: size as u64,
        })
    }

    /// Get a value from the store (deserializes from JSON).
    #[tracing::instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn get(&self, key: &str) -> Result<Option<KvValue<T>>> {
        match self.store.entry(key).await {
            Ok(Some(entry)) => {
                let size = entry.value.len();
                let deserialized = serde_json::from_slice(&entry.value)?;
                tracing::debug!(
                    target: TRACING_TARGET_KV,
                    key = %key,
                    size_bytes = size,
                    revision = entry.revision,
                    "Retrieved value from KV store"
                );
                Ok(Some(KvValue {
                    key: key.to_string(),
                    value: deserialized,
                    revision: entry.revision,
                    size: size as u64,
                    created: entry.created.into(),
                }))
            }
            Ok(None) => {
                tracing::debug!(
                    target: TRACING_TARGET_KV,
                    key = %key,
                    "Key not found in KV store"
                );
                Ok(None)
            }
            Err(e) => Err(Error::operation("kv_get", e.to_string())),
        }
    }

    /// Delete a key from the store.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn delete(&self, key: &str) -> Result<()> {
        self.store
            .purge(key)
            .await
            .map_err(|e| Error::operation("kv_delete", e.to_string()))?;

        tracing::debug!(
            target: TRACING_TARGET_KV,
            key = %key,
            "Deleted key from KV store"
        );
        Ok(())
    }

    /// Check if a key exists in the store.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn exists(&self, key: &str) -> Result<bool> {
        match self.store.get(key).await {
            Ok(Some(_)) => {
                tracing::debug!(
                    target: TRACING_TARGET_KV,
                    key = %key,
                    exists = true,
                    "Checked key existence"
                );
                Ok(true)
            }
            Ok(None) => {
                tracing::debug!(
                    target: TRACING_TARGET_KV,
                    key = %key,
                    exists = false,
                    "Checked key existence"
                );
                Ok(false)
            }
            Err(e) => Err(Error::operation("kv_exists", e.to_string())),
        }
    }

    /// Touches a key to reset its TTL by re-putting the same value.
    ///
    /// Returns an error if the key doesn't exist.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn touch(&self, key: &str) -> Result<KvEntry> {
        let kv_value = self
            .get(key)
            .await?
            .ok_or_else(|| Error::operation("kv_touch", format!("key not found: {key}")))?;

        let entry = self.put(key, &kv_value.value).await?;

        tracing::debug!(
            target: TRACING_TARGET_KV,
            key = %key,
            revision = entry.revision,
            "Touched key (TTL reset)"
        );

        Ok(entry)
    }

    /// Get all keys in the bucket.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn keys(&self) -> Result<Vec<String>> {
        let mut keys = Vec::new();
        let mut key_stream = self
            .store
            .keys()
            .await
            .map_err(|e| Error::operation("kv_keys", e.to_string()))?;

        while let Some(key) = key_stream.next().await {
            match key {
                Ok(k) => keys.push(k),
                Err(e) => {
                    tracing::warn!(
                        target: TRACING_TARGET_KV,
                        error = %e,
                        "Error reading key from bucket"
                    );
                }
            }
        }

        tracing::debug!(
            target: TRACING_TARGET_KV,
            count = keys.len(),
            bucket = %self.store.name,
            "Retrieved keys from bucket"
        );
        Ok(keys)
    }

    /// Purge all keys in the bucket.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn purge_all(&self) -> Result<()> {
        let keys = self.keys().await?;
        let count = keys.len();
        for key in keys {
            self.delete(&key).await?;
        }
        tracing::debug!(
            target: TRACING_TARGET_KV,
            count = count,
            bucket = %self.store.name,
            "Purged all keys from bucket"
        );
        Ok(())
    }

    /// Get the underlying store reference
    pub fn inner(&self) -> &kv::Store {
        &self.store
    }

    /// Set/update a value (alias for put for consistency with cache interface).
    #[tracing::instrument(skip(self, value), target = TRACING_TARGET_KV)]
    pub async fn set(&self, key: &str, value: &T) -> Result<KvEntry> {
        self.put(key, value).await
    }

    /// Get a value and extract just the data (convenience method)
    #[tracing::instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn get_value(&self, key: &str) -> Result<Option<T>> {
        Ok(self.get(key).await?.map(|kv_value| kv_value.value))
    }

    /// Put multiple values in a batch operation.
    #[tracing::instrument(skip(self, items), target = TRACING_TARGET_KV)]
    pub async fn put_batch(&self, items: &[(&str, &T)]) -> Result<Vec<KvEntry>> {
        let mut results = Vec::with_capacity(items.len());

        for (key, value) in items {
            let entry = self.put(key, value).await?;
            results.push(entry);
        }

        tracing::debug!(
            target: TRACING_TARGET_KV,
            count = items.len(),
            "Batch put completed"
        );

        Ok(results)
    }

    /// Get multiple values in a batch operation.
    #[tracing::instrument(skip(self, keys), target = TRACING_TARGET_KV)]
    pub async fn get_batch(&self, keys: &[&str]) -> Result<HashMap<String, KvValue<T>>> {
        let mut results = HashMap::with_capacity(keys.len());

        for key in keys {
            if let Some(value) = self.get(key).await? {
                results.insert(key.to_string(), value);
            }
        }

        tracing::debug!(
            target: TRACING_TARGET_KV,
            requested = keys.len(),
            found = results.len(),
            "Batch get completed"
        );

        Ok(results)
    }

    /// Update a value only if the revision matches (optimistic concurrency).
    #[tracing::instrument(skip(self, value), target = TRACING_TARGET_KV)]
    pub async fn update(&self, key: &str, value: &T, revision: u64) -> Result<KvEntry> {
        let json = serde_json::to_vec(value)?;
        let size = json.len();
        let new_revision = self
            .store
            .update(key, json.into(), revision)
            .await
            .map_err(|e| Error::operation("kv_update", e.to_string()))?;

        tracing::debug!(
            target: TRACING_TARGET_KV,
            key = %key,
            old_revision = revision,
            new_revision = new_revision,
            size_bytes = size,
            "Updated value in KV store"
        );

        Ok(KvEntry {
            key: key.to_string(),
            revision: new_revision,
            size: size as u64,
        })
    }

    /// Get or compute a value using the cache-aside pattern.
    #[tracing::instrument(skip(self, compute_fn), target = TRACING_TARGET_KV)]
    pub async fn get_or_compute<F, Fut>(&self, key: &str, compute_fn: F) -> Result<T>
    where
        F: FnOnce() -> Fut + Send,
        Fut: std::future::Future<Output = Result<T>> + Send,
        T: Clone,
    {
        // Try to get from store first
        if let Some(existing) = self.get_value(key).await? {
            tracing::debug!(
                target: TRACING_TARGET_KV,
                key = %key,
                "Found existing value in store"
            );
            return Ok(existing);
        }

        // Value not found, compute it
        tracing::debug!(
            target: TRACING_TARGET_KV,
            key = %key,
            "Value not found, computing new value"
        );
        let value = compute_fn().await?;

        // Store the computed value
        self.put(key, &value).await?;

        Ok(value)
    }
}

/// KV entry metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KvEntry {
    pub key: String,
    pub revision: u64,
    pub size: u64,
}

/// KV value with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KvValue<T> {
    pub key: String,
    pub value: T,
    pub revision: u64,
    pub size: u64,
    pub created: std::time::SystemTime,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    #[allow(dead_code)]
    struct TestData {
        id: u64,
        name: String,
    }

    // Note: These tests would require a running NATS server with JetStream enabled
    // They're marked as ignored for now

    #[test]
    #[ignore]
    fn test_kv_operations() {
        // Would test put/get/delete operations
    }

    #[test]
    fn test_kv_entry_creation() {
        let entry = KvEntry {
            key: "test_key".to_string(),
            revision: 1,
            size: 100,
        };

        assert_eq!(entry.key, "test_key");
        assert_eq!(entry.revision, 1);
        assert_eq!(entry.size, 100);
    }
}
