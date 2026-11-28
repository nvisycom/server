//! Type-safe NATS KV store wrapper.

use std::marker::PhantomData;

use async_nats::jetstream::kv::{CreateErrorKind, PurgeErrorKind};
use async_nats::jetstream::{self, kv};

use super::core::{KvBucket, TokenKey};
use crate::{Error, Result, TRACING_TARGET_KV};

/// Type-safe NATS KV store wrapper.
///
/// Generic only over the bucket `B`, which fixes the value type
/// ([`KvBucket::Value`]) as well as the name, description, and TTL; every entry
/// is keyed by a [`TokenKey`].
#[derive(Clone)]
pub struct KvStore<B: KvBucket> {
    store: kv::Store,
    _bucket: PhantomData<B>,
}

impl<B: KvBucket> KvStore<B> {
    /// Opens the bucket, which [`ensure`](Self::ensure) created at startup.
    #[tracing::instrument(skip(jetstream), target = TRACING_TARGET_KV)]
    pub(crate) async fn open(jetstream: &jetstream::Context) -> Result<Self> {
        let store = jetstream
            .get_key_value(B::NAME)
            .await
            .map_err(|e| Error::operation("kv_open_bucket", format!("{}: {e}", B::NAME)))?;
        Ok(Self {
            store,
            _bucket: PhantomData,
        })
    }

    /// Creates the bucket from its configuration if it does not exist yet.
    #[tracing::instrument(skip(jetstream), target = TRACING_TARGET_KV)]
    pub(crate) async fn ensure(jetstream: &jetstream::Context) -> Result<()> {
        if jetstream.get_key_value(B::NAME).await.is_ok() {
            return Ok(());
        }
        tracing::debug!(target: TRACING_TARGET_KV, bucket = %B::NAME, "Creating KV bucket");
        jetstream
            .create_key_value(kv::Config {
                bucket: B::NAME.to_owned(),
                description: B::DESCRIPTION.to_owned(),
                max_age: B::TTL,
                ..Default::default()
            })
            .await
            .map_err(|e| Error::operation("kv_create_bucket", format!("{}: {e}", B::NAME)))?;
        Ok(())
    }

    /// Puts a value into the store, replacing any existing one.
    ///
    /// # Errors
    ///
    /// - `Serialization` if `value` cannot be serialized to JSON.
    /// - `Operation` if the KV put fails (e.g. the connection is down or the
    ///   server rejects the write).
    #[tracing::instrument(skip(self, value), target = TRACING_TARGET_KV)]
    pub async fn put(&self, key: &TokenKey, value: &B::Value) -> Result<()> {
        let json = serde_json::to_vec(value)?;
        self.store
            .put(key.to_string(), json.into())
            .await
            .map_err(|e| Error::operation("kv_put", e.to_string()))?;
        Ok(())
    }

    /// Atomically creates a value only if the key does not already exist.
    ///
    /// Returns `Ok(true)` if this call created the entry, `Ok(false)` if the key
    /// was already present. This is the put-if-absent primitive for distributed
    /// locks: combined with a bucket TTL, exactly one caller wins the create and
    /// the entry expires if the winner dies.
    ///
    /// # Errors
    ///
    /// - `Serialization` if `value` cannot be serialized to JSON.
    /// - `Operation` if the create fails for any reason other than the key
    ///   already existing (e.g. network, ack, or publish failure).
    #[tracing::instrument(skip(self, value), target = TRACING_TARGET_KV)]
    pub async fn create(&self, key: &TokenKey, value: &B::Value) -> Result<bool> {
        let json = serde_json::to_vec(value)?;
        match self.store.create(key.to_string(), json.into()).await {
            Ok(_revision) => Ok(true),
            // A create against an existing key is the "lock already held" case,
            // not an error. Any other failure is a real infrastructure error and
            // must not be masqueraded as a lost lock.
            Err(err) if err.kind() == CreateErrorKind::AlreadyExists => Ok(false),
            Err(err) => Err(Error::operation("kv_create", err.to_string())),
        }
    }

    /// Reads and removes a value in one step, for single-use entries (an OAuth
    /// state, a reauth proof): of concurrent callers, exactly one gets the value.
    ///
    /// The entry is purged only if its revision is still the one read, so a
    /// caller that loses the race sees `Ok(None)` rather than a value another
    /// caller already consumed.
    ///
    /// # Errors
    ///
    /// - `Serialization` if the stored entry cannot be deserialized.
    /// - `Operation` if the read or the conditional purge fails for any reason
    ///   other than a lost race (e.g. network, ack, or publish failure).
    #[tracing::instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn take(&self, key: &TokenKey) -> Result<Option<B::Value>> {
        let key = key.to_string();
        let entry = self
            .store
            .entry(&key)
            .await
            .map_err(|e| Error::operation("kv_get", e.to_string()))?;
        // A purged or deleted key leaves a tombstone entry, which is absent.
        let Some(entry) = entry.filter(|entry| entry.operation == kv::Operation::Put) else {
            return Ok(None);
        };
        let value = serde_json::from_slice(&entry.value)?;

        match self
            .store
            .purge_expect_revision(&key, Some(entry.revision))
            .await
        {
            Ok(()) => Ok(Some(value)),
            // Only a revision mismatch means another caller consumed it first;
            // any other failure must propagate, since masking it as `None` would
            // leave a single-use token unconsumed and replayable.
            Err(err) if err.kind() == PurgeErrorKind::WrongLastRevision => {
                tracing::debug!(target: TRACING_TARGET_KV, key = %key, "Single-use take lost the race");
                Ok(None)
            }
            Err(err) => Err(Error::operation("kv_take", err.to_string())),
        }
    }
}

#[cfg(all(test, feature = "test_util"))]
mod tests {
    use std::time::Duration;

    use crate::kv::TokenKey;
    use crate::test_util::TestNats;

    crate::kv_bucket! {
        /// A bucket for exercising the store.
        TestBucket: u64 {
            name: "test_bucket",
            description: "Test bucket",
            ttl: Duration::from_mins(1),
        }
    }

    #[tokio::test]
    async fn create_is_put_if_absent_and_take_is_single_use() {
        let nats = TestNats::start().await;
        nats.client.ensure_kv_bucket::<TestBucket>().await.unwrap();
        let store = nats.client.kv_store::<TestBucket>().await.unwrap();
        let key: TokenKey = "lock.1".parse().unwrap();

        assert!(
            store.create(&key, &7).await.unwrap(),
            "the first create wins"
        );
        assert!(
            !store.create(&key, &8).await.unwrap(),
            "the key is already held"
        );

        assert_eq!(store.take(&key).await.unwrap(), Some(7));
        assert_eq!(
            store.take(&key).await.unwrap(),
            None,
            "a taken value is gone"
        );

        store.put(&key, &9).await.unwrap();
        assert_eq!(store.take(&key).await.unwrap(), Some(9));
    }
}
