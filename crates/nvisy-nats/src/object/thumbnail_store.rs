//! Thumbnail store for NATS object storage.

use async_nats::jetstream;
use derive_more::{Deref, DerefMut};

use super::document_key::DocumentKey;
use super::object_data::{GetResult, PutResult};
use super::object_store::ObjectStore;
use super::thumbnail_bucket::{THUMBNAIL_BUCKET, THUMBNAIL_MAX_AGE};
use crate::Result;

/// A thumbnail store that manages document thumbnails in NATS object storage.
///
/// Uses [`DocumentKey`] for addressing (same key format as document files).
#[derive(Clone, Deref, DerefMut)]
pub struct ThumbnailStore {
    #[deref]
    #[deref_mut]
    inner: ObjectStore,
}

impl ThumbnailStore {
    /// Creates a new thumbnail store.
    pub async fn new(jetstream: &jetstream::Context) -> Result<Self> {
        let inner = ObjectStore::new(jetstream, THUMBNAIL_BUCKET, THUMBNAIL_MAX_AGE).await?;
        Ok(Self { inner })
    }

    /// Streams thumbnail data to the store while computing SHA-256 hash on-the-fly.
    pub async fn put<R>(&self, key: &DocumentKey, reader: R) -> Result<PutResult>
    where
        R: tokio::io::AsyncRead + Unpin,
    {
        self.inner.put(&key.to_string(), reader).await
    }

    /// Gets a thumbnail from the store as a stream.
    ///
    /// Returns `None` if the thumbnail doesn't exist.
    pub async fn get(&self, key: &DocumentKey) -> Result<Option<GetResult>> {
        self.inner.get(&key.to_string()).await
    }

    /// Deletes a thumbnail from the store.
    pub async fn delete(&self, key: &DocumentKey) -> Result<()> {
        self.inner.delete(&key.to_string()).await
    }

    /// Checks if a thumbnail exists.
    pub async fn exists(&self, key: &DocumentKey) -> Result<bool> {
        self.inner.exists(&key.to_string()).await
    }

    /// Returns the bucket name.
    #[inline]
    pub fn bucket(&self) -> &'static str {
        THUMBNAIL_BUCKET
    }
}
