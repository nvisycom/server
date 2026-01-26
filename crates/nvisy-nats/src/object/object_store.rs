//! Generic object store for NATS JetStream.

use std::marker::PhantomData;
use std::sync::Arc;

use async_nats::jetstream;
use async_nats::jetstream::context::ObjectStoreErrorKind;
use async_nats::jetstream::object_store::{self, ObjectInfo};
use tokio::io::AsyncRead;

use super::hashing_reader::HashingReader;
use super::object_bucket::ObjectBucket;
use super::object_data::{GetResult, PutResult};
use super::object_key::ObjectKey;
use crate::{Error, Result};

/// Tracing target for object store operations.
const TRACING_TARGET: &str = "nvisy_nats::object_store";

/// A type-safe object store that manages objects in NATS object storage.
///
/// This store provides streaming upload capabilities with on-the-fly
/// SHA-256 hash computation.
///
/// The store is generic over:
/// - `B`: The bucket type (determines storage location and TTL)
/// - `K`: The key type (determines how objects are addressed)
#[derive(Clone)]
pub struct ObjectStore<B, K>
where
    B: ObjectBucket,
    K: ObjectKey,
{
    inner: Arc<object_store::ObjectStore>,
    _marker: PhantomData<(B, K)>,
}

impl<B, K> ObjectStore<B, K>
where
    B: ObjectBucket,
    K: ObjectKey,
{
    /// Creates a new object store for the specified bucket type.
    pub(crate) async fn new(jetstream: &jetstream::Context) -> Result<Self> {
        tracing::debug!(
            target: TRACING_TARGET,
            bucket = %B::NAME,
            "Initializing object store"
        );

        let store = match jetstream.get_object_store(B::NAME).await {
            Ok(store) => {
                tracing::debug!(
                    target: TRACING_TARGET,
                    bucket = %B::NAME,
                    "Retrieved existing object store"
                );
                store
            }
            Err(e) if matches!(e.kind(), ObjectStoreErrorKind::GetStore) => {
                let config = object_store::Config {
                    bucket: B::NAME.to_string(),
                    max_age: B::MAX_AGE.unwrap_or_default(),
                    ..Default::default()
                };

                tracing::info!(
                    target: TRACING_TARGET,
                    bucket = %B::NAME,
                    "Creating new object store"
                );

                jetstream.create_object_store(config).await.map_err(|e| {
                    tracing::error!(
                        target: TRACING_TARGET,
                        bucket = %B::NAME,
                        error = %e,
                        "Failed to create object store"
                    );
                    Error::operation("create_object_store", e.to_string())
                })?
            }
            Err(e) => {
                tracing::error!(
                    target: TRACING_TARGET,
                    bucket = %B::NAME,
                    error = %e,
                    "Failed to get object store"
                );
                return Err(Error::operation("get_object_store", e.to_string()));
            }
        };

        Ok(Self {
            inner: Arc::new(store),
            _marker: PhantomData,
        })
    }

    /// Returns the bucket name.
    #[inline]
    pub fn bucket(&self) -> &'static str {
        B::NAME
    }

    /// Streams data to the store while computing SHA-256 hash on-the-fly.
    ///
    /// This method does not buffer the entire content in memory, making it
    /// suitable for large file uploads.
    pub async fn put<R>(&self, key: &K, reader: R) -> Result<PutResult>
    where
        R: AsyncRead + Unpin,
    {
        let key_str = key.to_string();

        tracing::debug!(
            target: TRACING_TARGET,
            key = %key_str,
            bucket = %B::NAME,
            "Starting streaming upload"
        );

        let meta = object_store::ObjectMetadata {
            name: key_str.clone(),
            ..Default::default()
        };

        let mut hashing_reader = HashingReader::new(reader);

        let info = self
            .inner
            .put(meta, &mut hashing_reader)
            .await
            .map_err(|e| {
                tracing::error!(
                    target: TRACING_TARGET,
                    key = %key_str,
                    error = %e,
                    "Failed to upload object"
                );
                Error::operation("put", e.to_string())
            })?;

        let sha256 = hashing_reader.finalize();

        tracing::info!(
            target: TRACING_TARGET,
            key = %key_str,
            size = info.size,
            nuid = %info.nuid,
            "Streaming upload complete"
        );

        Ok(PutResult::new(info.size as u64, sha256.to_vec(), info.nuid))
    }

    /// Gets an object from the store as a stream.
    ///
    /// Returns `None` if the object doesn't exist.
    /// The returned reader implements `AsyncRead` for streaming the content.
    pub async fn get(&self, key: &K) -> Result<Option<GetResult>> {
        let key_str = key.to_string();

        tracing::debug!(
            target: TRACING_TARGET,
            key = %key_str,
            bucket = %B::NAME,
            "Getting object"
        );

        // First get the object info
        let info = match self.info(key).await? {
            Some(info) => info,
            None => return Ok(None),
        };

        match self.inner.get(&key_str).await {
            Ok(reader) => {
                tracing::debug!(
                    target: TRACING_TARGET,
                    key = %key_str,
                    size = info.size,
                    "Object stream opened"
                );

                Ok(Some(GetResult::new(reader, info)))
            }
            Err(e) => {
                let error_str = e.to_string();
                if error_str.contains("not found") || error_str.contains("no message found") {
                    tracing::debug!(
                        target: TRACING_TARGET,
                        key = %key_str,
                        "Object not found"
                    );
                    Ok(None)
                } else {
                    tracing::error!(
                        target: TRACING_TARGET,
                        key = %key_str,
                        error = %e,
                        "Failed to get object"
                    );
                    Err(Error::operation("get", e.to_string()))
                }
            }
        }
    }

    /// Gets object info without downloading the content.
    pub async fn info(&self, key: &K) -> Result<Option<ObjectInfo>> {
        let key_str = key.to_string();

        match self.inner.info(&key_str).await {
            Ok(info) => Ok(Some(info)),
            Err(e) => {
                let error_str = e.to_string();
                if error_str.contains("not found") {
                    Ok(None)
                } else {
                    Err(Error::operation("info", e.to_string()))
                }
            }
        }
    }

    /// Deletes an object from the store.
    pub async fn delete(&self, key: &K) -> Result<()> {
        let key_str = key.to_string();

        tracing::debug!(
            target: TRACING_TARGET,
            key = %key_str,
            bucket = %B::NAME,
            "Deleting object"
        );

        self.inner.delete(&key_str).await.map_err(|e| {
            tracing::error!(
                target: TRACING_TARGET,
                key = %key_str,
                error = %e,
                "Failed to delete object"
            );
            Error::operation("delete", e.to_string())
        })?;

        tracing::info!(
            target: TRACING_TARGET,
            key = %key_str,
            "Object deleted"
        );

        Ok(())
    }

    /// Checks if an object exists.
    pub async fn exists(&self, key: &K) -> Result<bool> {
        Ok(self.info(key).await?.is_some())
    }
}
