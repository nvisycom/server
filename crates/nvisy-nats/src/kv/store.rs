//! Generic NATS KV store wrapper.

use std::time::Duration;

use async_nats::jetstream::{self, kv};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument, warn};

use crate::{Error, Result, TRACING_TARGET_KV};

/// Generic KV store wrapper
#[derive(Clone)]
pub struct KvStore {
    store: kv::Store,
}

impl KvStore {
    /// Create or get a KV bucket
    #[instrument(skip(jetstream), target = TRACING_TARGET_KV)]
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
                debug!(
                    target: TRACING_TARGET_KV,
                    bucket = %bucket_name,
                    "Using existing KV bucket"
                );
                store
            }
            Err(_) => {
                // Bucket doesn't exist, create it
                debug!(
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

        Ok(Self { store })
    }

    /// Get the bucket name
    pub fn bucket_name(&self) -> &str {
        &self.store.name
    }

    /// Put a value (serializes to JSON)
    #[instrument(skip(self, value), target = TRACING_TARGET_KV)]
    pub async fn put<T: Serialize>(&self, key: &str, value: &T) -> Result<u64> {
        let json = serde_json::to_vec(value)?;
        let size = json.len();
        let revision = self
            .store
            .put(key, json.into())
            .await
            .map_err(|e| Error::operation("kv_put", e.to_string()))?;

        debug!(
            target: TRACING_TARGET_KV,
            key = %key,
            revision = revision,
            size_bytes = size,
            "Put value to KV store"
        );
        Ok(revision)
    }

    /// Get a value (deserializes from JSON)
    #[instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn get<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Result<Option<T>> {
        match self.store.get(key).await {
            Ok(Some(value)) => {
                let size = value.len();
                let deserialized = serde_json::from_slice(&value)?;
                debug!(
                    target: TRACING_TARGET_KV,
                    key = %key,
                    size_bytes = size,
                    "Retrieved value from KV store"
                );
                Ok(Some(deserialized))
            }
            Ok(None) => {
                debug!(
                    target: TRACING_TARGET_KV,
                    key = %key,
                    "Key not found in KV store"
                );
                Ok(None)
            }
            Err(e) => Err(Error::operation("kv_get", e.to_string())),
        }
    }

    /// Delete a key
    #[instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn delete(&self, key: &str) -> Result<()> {
        self.store
            .purge(key)
            .await
            .map_err(|e| Error::operation("kv_delete", e.to_string()))?;

        debug!(
            target: TRACING_TARGET_KV,
            key = %key,
            "Deleted key from KV store"
        );
        Ok(())
    }

    /// Check if a key exists
    #[instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn exists(&self, key: &str) -> Result<bool> {
        match self.store.get(key).await {
            Ok(Some(_)) => {
                debug!(
                    target: TRACING_TARGET_KV,
                    key = %key,
                    exists = true,
                    "Checked key existence"
                );
                Ok(true)
            }
            Ok(None) => {
                debug!(
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

    /// Get all keys in the bucket
    #[instrument(skip(self), target = TRACING_TARGET_KV)]
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
                    warn!(
                        target: TRACING_TARGET_KV,
                        error = %e,
                        "Error reading key from bucket"
                    );
                }
            }
        }

        debug!(
            target: TRACING_TARGET_KV,
            count = keys.len(),
            bucket = %self.store.name,
            "Retrieved keys from bucket"
        );
        Ok(keys)
    }

    /// Purge all keys in the bucket
    #[instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn purge_all(&self) -> Result<()> {
        let keys = self.keys().await?;
        let count = keys.len();
        for key in keys {
            self.delete(&key).await?;
        }
        debug!(
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
    // They're marked as ignored for now

    #[test]
    #[ignore]
    fn test_kv_operations() {
        // Would test put/get/delete operations
    }
}
