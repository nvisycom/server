//! Generic object store wrapper for NATS JetStream.

use std::sync::Arc;
use std::time::Duration;

use async_nats::jetstream;
use async_nats::jetstream::context::ObjectStoreErrorKind;
use async_nats::jetstream::object_store::{self, ObjectInfo};
use tokio::io::AsyncRead;

use super::hashing_reader::HashingReader;
use super::object_data::{GetResult, PutResult};
use crate::{Error, Result};

/// Tracing target for object store operations.
const TRACING_TARGET: &str = "nvisy_nats::object_store";

/// A generic object store that manages files in NATS object storage.
///
/// This store provides streaming upload capabilities with on-the-fly
/// SHA-256 hash computation.
#[derive(Clone)]
pub struct ObjectStore {
    inner: Arc<object_store::ObjectStore>,
    bucket: Arc<String>,
}

impl ObjectStore {
    /// Creates a new object store for the specified bucket.
    ///
    /// If `max_age` is `None`, objects will not expire.
    pub async fn new(
        jetstream: &jetstream::Context,
        bucket: impl Into<String>,
        max_age: Option<Duration>,
    ) -> Result<Self> {
        let bucket = bucket.into();

        tracing::debug!(
            target: TRACING_TARGET,
            bucket = %bucket,
            "Initializing object store"
        );

        let store = match jetstream.get_object_store(&bucket).await {
            Ok(store) => {
                tracing::debug!(
                    target: TRACING_TARGET,
                    bucket = %bucket,
                    "Retrieved existing object store"
                );
                store
            }
            Err(e) if matches!(e.kind(), ObjectStoreErrorKind::GetStore) => {
                let config = object_store::Config {
                    bucket: bucket.clone(),
                    max_age: max_age.unwrap_or_default(),
                    ..Default::default()
                };

                tracing::info!(
                    target: TRACING_TARGET,
                    bucket = %bucket,
                    "Creating new object store"
                );

                jetstream.create_object_store(config).await.map_err(|e| {
                    tracing::error!(
                        target: TRACING_TARGET,
                        bucket = %bucket,
                        error = %e,
                        "Failed to create object store"
                    );
                    Error::operation("create_object_store", e.to_string())
                })?
            }
            Err(e) => {
                tracing::error!(
                    target: TRACING_TARGET,
                    bucket = %bucket,
                    error = %e,
                    "Failed to get object store"
                );
                return Err(Error::operation("get_object_store", e.to_string()));
            }
        };

        Ok(Self {
            inner: Arc::new(store),
            bucket: Arc::new(bucket),
        })
    }

    /// Returns the bucket name.
    pub fn bucket(&self) -> &str {
        &self.bucket
    }

    /// Streams data to the store while computing SHA-256 hash on-the-fly.
    ///
    /// This method does not buffer the entire content in memory, making it
    /// suitable for large file uploads.
    pub async fn put<R>(&self, key: &str, reader: R) -> Result<PutResult>
    where
        R: AsyncRead + Unpin,
    {
        tracing::debug!(
            target: TRACING_TARGET,
            key = %key,
            bucket = %self.bucket,
            "Starting streaming upload"
        );

        let meta = object_store::ObjectMetadata {
            name: key.to_string(),
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
                    key = %key,
                    error = %e,
                    "Failed to upload object"
                );
                Error::operation("put", e.to_string())
            })?;

        let sha256 = hashing_reader.finalize();
        let sha256_hex = hex::encode(sha256);

        tracing::info!(
            target: TRACING_TARGET,
            key = %key,
            size = info.size,
            sha256 = %sha256_hex,
            nuid = %info.nuid,
            "Streaming upload complete"
        );

        Ok(PutResult::new(
            info.size as u64,
            sha256.to_vec(),
            sha256_hex,
            info.nuid,
        ))
    }

    /// Gets an object from the store as a stream.
    ///
    /// Returns `None` if the object doesn't exist.
    /// The returned reader implements `AsyncRead` for streaming the content.
    pub async fn get(&self, key: &str) -> Result<Option<GetResult>> {
        tracing::debug!(
            target: TRACING_TARGET,
            key = %key,
            bucket = %self.bucket,
            "Getting object"
        );

        // First get the object info
        let info = match self.info(key).await? {
            Some(info) => info,
            None => return Ok(None),
        };

        match self.inner.get(key).await {
            Ok(reader) => {
                tracing::debug!(
                    target: TRACING_TARGET,
                    key = %key,
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
                        key = %key,
                        "Object not found"
                    );
                    Ok(None)
                } else {
                    tracing::error!(
                        target: TRACING_TARGET,
                        key = %key,
                        error = %e,
                        "Failed to get object"
                    );
                    Err(Error::operation("get", e.to_string()))
                }
            }
        }
    }

    /// Gets object info without downloading the content.
    pub async fn info(&self, key: &str) -> Result<Option<ObjectInfo>> {
        match self.inner.info(key).await {
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
    pub async fn delete(&self, key: &str) -> Result<()> {
        tracing::debug!(
            target: TRACING_TARGET,
            key = %key,
            bucket = %self.bucket,
            "Deleting object"
        );

        self.inner.delete(key).await.map_err(|e| {
            tracing::error!(
                target: TRACING_TARGET,
                key = %key,
                error = %e,
                "Failed to delete object"
            );
            Error::operation("delete", e.to_string())
        })?;

        tracing::info!(
            target: TRACING_TARGET,
            key = %key,
            "Object deleted"
        );

        Ok(())
    }

    /// Checks if an object exists.
    pub async fn exists(&self, key: &str) -> Result<bool> {
        Ok(self.info(key).await?.is_some())
    }
}
