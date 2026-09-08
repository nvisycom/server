//! Type-safe NATS KV store wrapper.

use std::future::Future;
use std::marker::PhantomData;
use std::time::{Duration, SystemTime};

use async_nats::jetstream::{self, kv};
use futures::StreamExt;
use serde::{Deserialize, Serialize};

use super::core::KvBucket;
use crate::{Error, Result, TRACING_TARGET_KV};

/// Type-safe NATS KV store wrapper.
///
/// Generic only over the bucket `B`, which fixes the key type ([`KvBucket::Key`])
/// and value type ([`KvBucket::Value`]) as well as the name, description, and TTL.
#[derive(Clone)]
pub struct KvStore<B: KvBucket> {
    store: kv::Store,
    _bucket: PhantomData<B>,
}

impl<B: KvBucket> KvStore<B> {
    /// Create or get a KV bucket using the bucket configuration.
    #[tracing::instrument(skip(jetstream), target = TRACING_TARGET_KV)]
    pub(crate) async fn new(jetstream: &jetstream::Context) -> Result<Self> {
        Self::with_ttl(jetstream, B::TTL.unwrap_or_default()).await
    }

    /// Create or get a KV bucket with custom TTL.
    #[tracing::instrument(skip(jetstream), target = TRACING_TARGET_KV)]
    pub(crate) async fn with_ttl(jetstream: &jetstream::Context, ttl: Duration) -> Result<Self> {
        let config = kv::Config {
            bucket: B::NAME.to_string(),
            description: B::DESCRIPTION.to_string(),
            max_age: ttl,
            ..Default::default()
        };

        let store = match jetstream.get_key_value(B::NAME).await {
            Ok(store) => {
                tracing::trace!(
                    target: TRACING_TARGET_KV,
                    bucket = %B::NAME,
                    "Using existing KV bucket"
                );
                store
            }
            Err(_) => {
                tracing::debug!(
                    target: TRACING_TARGET_KV,
                    bucket = %B::NAME,
                    ttl_secs = ttl.as_secs(),
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
            _bucket: PhantomData,
        })
    }

    /// Returns the bucket name.
    #[inline]
    pub fn bucket_name(&self) -> &'static str {
        B::NAME
    }

    /// Put a value into the store.
    #[tracing::instrument(skip(self, value), target = TRACING_TARGET_KV)]
    pub async fn put(&self, key: &B::Key, value: &B::Value) -> Result<KvEntry> {
        let key_str = key.to_string();
        let json = serde_json::to_vec(value)?;
        let size = json.len();
        let revision = self
            .store
            .put(&key_str, json.into())
            .await
            .map_err(|e| Error::operation("kv_put", e.to_string()))?;

        tracing::debug!(
            target: TRACING_TARGET_KV,
            key = %key_str,
            revision = revision,
            size_bytes = size,
            "Put value to KV store"
        );

        Ok(KvEntry {
            key: key_str,
            revision,
            size: size as u64,
        })
    }

    /// Atomically create a value only if the key does not already exist.
    ///
    /// Returns `Ok(true)` if this call created the entry, `Ok(false)` if the key
    /// was already present. This is the put-if-absent primitive for distributed
    /// locks / leader election: combined with a bucket TTL, exactly one caller
    /// wins the create and the entry auto-expires if the winner dies.
    #[tracing::instrument(skip(self, value), target = TRACING_TARGET_KV)]
    pub async fn create(&self, key: &B::Key, value: &B::Value) -> Result<bool> {
        use async_nats::jetstream::kv::CreateErrorKind;

        let key_str = key.to_string();
        let json = serde_json::to_vec(value)?;
        match self.store.create(&key_str, json.into()).await {
            Ok(_revision) => Ok(true),
            // A create against an existing key is the "lock already held" case,
            // not an error. Any other failure (network, ack, publish) is a real
            // infrastructure error and must be propagated rather than masqueraded
            // as a lost lock.
            Err(err) if err.kind() == CreateErrorKind::AlreadyExists => Ok(false),
            Err(err) => Err(Error::operation("kv_create", err.to_string())),
        }
    }

    /// Get a value from the store.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn get(&self, key: &B::Key) -> Result<Option<KvValue<B::Value>>> {
        let key_str = key.to_string();
        match self.store.entry(&key_str).await {
            Ok(Some(entry)) => {
                let size = entry.value.len();
                let deserialized = serde_json::from_slice(&entry.value)?;
                tracing::debug!(
                    target: TRACING_TARGET_KV,
                    key = %key_str,
                    size_bytes = size,
                    revision = entry.revision,
                    "Retrieved value from KV store"
                );
                Ok(Some(KvValue {
                    key: key_str,
                    value: deserialized,
                    revision: entry.revision,
                    size: size as u64,
                    created: entry.created.into(),
                }))
            }
            Ok(None) => {
                tracing::debug!(
                    target: TRACING_TARGET_KV,
                    key = %key_str,
                    "Key not found in KV store"
                );
                Ok(None)
            }
            Err(e) => Err(Error::operation("kv_get", e.to_string())),
        }
    }

    /// Get a value, returning just the data.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn get_value(&self, key: &B::Key) -> Result<Option<B::Value>> {
        Ok(self.get(key).await?.map(|kv| kv.value))
    }

    /// Atomically consume a single-use entry: read its value and delete it in a
    /// way that only one caller can win.
    ///
    /// Returns `Some(value)` to exactly one caller and `None` to everyone else,
    /// even under concurrent calls for the same key. The delete is conditioned on
    /// the revision the value was read at (`purge_expect_revision`), so a second
    /// racer whose read saw the same revision fails the conditional purge and gets
    /// `None` rather than re-consuming the entry. Use this for one-time tokens
    /// (OAuth/OIDC state, step-up proofs) where a plain get-then-delete would leave
    /// a replay window.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn take(&self, key: &B::Key) -> Result<Option<B::Value>> {
        use async_nats::jetstream::kv::PurgeErrorKind;

        let Some(entry) = self.get(key).await? else {
            return Ok(None);
        };
        let key_str = key.to_string();
        match self
            .store
            .purge_expect_revision(&key_str, Some(entry.revision))
            .await
        {
            Ok(()) => Ok(Some(entry.value)),
            // A concurrent caller consumed it first (the revision moved), so this
            // caller loses the race and sees nothing to consume. Only a revision
            // mismatch means "lost the race"; any other failure (network, ack,
            // publish) is a real error and must propagate — masking it as `None`
            // would silently leave a single-use token unconsumed and replayable.
            Err(err) if err.kind() == PurgeErrorKind::WrongLastRevision => {
                tracing::debug!(
                    target: TRACING_TARGET_KV,
                    key = %key_str,
                    error = %err,
                    "Single-use take lost the race; entry already consumed",
                );
                Ok(None)
            }
            Err(err) => Err(Error::operation("kv_take", err.to_string())),
        }
    }

    /// Delete a key from the store.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn delete(&self, key: &B::Key) -> Result<()> {
        let key_str = key.to_string();
        self.store
            .purge(&key_str)
            .await
            .map_err(|e| Error::operation("kv_delete", e.to_string()))?;

        tracing::debug!(
            target: TRACING_TARGET_KV,
            key = %key_str,
            "Deleted key from KV store"
        );
        Ok(())
    }

    /// Check if a key exists in the store.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn exists(&self, key: &B::Key) -> Result<bool> {
        let key_str = key.to_string();
        match self.store.get(&key_str).await {
            Ok(Some(_)) => Ok(true),
            Ok(None) => Ok(false),
            Err(e) => Err(Error::operation("kv_exists", e.to_string())),
        }
    }

    /// Touches a key to reset its TTL by re-putting the same value.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn touch(&self, key: &B::Key) -> Result<KvEntry> {
        let kv_value = self
            .get(key)
            .await?
            .ok_or_else(|| Error::operation("kv_touch", format!("key not found: {key}")))?;

        self.put(key, &kv_value.value).await
    }

    /// Get all keys in the bucket with the expected prefix.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn keys(&self) -> Result<Vec<B::Key>> {
        let mut keys = Vec::new();
        let mut key_stream = self
            .store
            .keys()
            .await
            .map_err(|e| Error::operation("kv_keys", e.to_string()))?;

        while let Some(key_result) = key_stream.next().await {
            match key_result {
                Ok(key_str) => {
                    if let Ok(key) = key_str.parse::<B::Key>() {
                        keys.push(key);
                    }
                }
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
            bucket = %B::NAME,
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
            bucket = %B::NAME,
            "Purged all keys from bucket"
        );
        Ok(())
    }

    /// Update a value only if the revision matches (optimistic concurrency).
    #[tracing::instrument(skip(self, value), target = TRACING_TARGET_KV)]
    pub async fn update(&self, key: &B::Key, value: &B::Value, revision: u64) -> Result<KvEntry> {
        let key_str = key.to_string();
        let json = serde_json::to_vec(value)?;
        let size = json.len();
        let new_revision = self
            .store
            .update(&key_str, json.into(), revision)
            .await
            .map_err(|e| Error::operation("kv_update", e.to_string()))?;

        tracing::debug!(
            target: TRACING_TARGET_KV,
            key = %key_str,
            old_revision = revision,
            new_revision = new_revision,
            size_bytes = size,
            "Updated value in KV store"
        );

        Ok(KvEntry {
            key: key_str,
            revision: new_revision,
            size: size as u64,
        })
    }

    /// Get or compute a value using the cache-aside pattern.
    #[tracing::instrument(skip(self, compute_fn), target = TRACING_TARGET_KV)]
    pub async fn get_or_compute<F, Fut>(&self, key: &B::Key, compute_fn: F) -> Result<B::Value>
    where
        F: FnOnce() -> Fut + Send,
        Fut: Future<Output = Result<B::Value>> + Send,
        B::Value: Clone,
    {
        if let Some(existing) = self.get_value(key).await? {
            return Ok(existing);
        }

        let value = compute_fn().await?;
        self.put(key, &value).await?;
        Ok(value)
    }

    /// Get the underlying store reference.
    pub fn inner(&self) -> &kv::Store {
        &self.store
    }
}

/// KV entry metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KvEntry {
    pub key: String,
    pub revision: u64,
    pub size: u64,
}

/// KV value with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KvValue<V> {
    pub key: String,
    pub value: V,
    pub revision: u64,
    pub size: u64,
    pub created: SystemTime,
}

#[cfg(test)]
mod tests {
    use super::*;

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
