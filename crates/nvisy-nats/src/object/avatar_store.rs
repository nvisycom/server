//! Avatar store for NATS object storage.

use async_nats::jetstream;
use derive_more::{Deref, DerefMut};

use super::avatar_bucket::{AVATAR_BUCKET, AVATAR_MAX_AGE};
use super::avatar_key::AvatarKey;
use super::object_data::{GetResult, PutResult};
use super::object_store::ObjectStore;
use crate::Result;

/// An avatar store that manages profile images in NATS object storage.
///
/// Uses [`AvatarKey`] for addressing (account ID based).
#[derive(Clone, Deref, DerefMut)]
pub struct AvatarStore {
    #[deref]
    #[deref_mut]
    inner: ObjectStore,
}

impl AvatarStore {
    /// Creates a new avatar store.
    pub async fn new(jetstream: &jetstream::Context) -> Result<Self> {
        let inner = ObjectStore::new(jetstream, AVATAR_BUCKET, AVATAR_MAX_AGE).await?;
        Ok(Self { inner })
    }

    /// Streams avatar data to the store while computing SHA-256 hash on-the-fly.
    pub async fn put<R>(&self, key: &AvatarKey, reader: R) -> Result<PutResult>
    where
        R: tokio::io::AsyncRead + Unpin,
    {
        self.inner.put(&key.to_string(), reader).await
    }

    /// Gets an avatar from the store as a stream.
    ///
    /// Returns `None` if the avatar doesn't exist.
    pub async fn get(&self, key: &AvatarKey) -> Result<Option<GetResult>> {
        self.inner.get(&key.to_string()).await
    }

    /// Deletes an avatar from the store.
    pub async fn delete(&self, key: &AvatarKey) -> Result<()> {
        self.inner.delete(&key.to_string()).await
    }

    /// Checks if an avatar exists.
    pub async fn exists(&self, key: &AvatarKey) -> Result<bool> {
        self.inner.exists(&key.to_string()).await
    }

    /// Returns the bucket name.
    #[inline]
    pub fn bucket(&self) -> &'static str {
        AVATAR_BUCKET
    }
}
