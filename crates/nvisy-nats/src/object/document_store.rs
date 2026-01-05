//! Document file store for NATS object storage.

use std::marker::PhantomData;

use async_nats::jetstream;
use derive_more::{Deref, DerefMut};

use super::document_bucket::DocumentBucket;
use super::document_key::DocumentKey;
use super::object_data::{GetResult, PutResult};
use super::object_store::ObjectStore;
use crate::Result;

/// A document file store that manages files in NATS object storage.
///
/// This is a specialized wrapper around [`ObjectStore`] that uses
/// [`DocumentKey`] for addressing and provides document-specific operations.
///
/// The store is generic over the bucket type, providing compile-time
/// type safety for bucket operations.
#[derive(Clone, Deref, DerefMut)]
pub struct DocumentStore<B: DocumentBucket> {
    #[deref]
    #[deref_mut]
    inner: ObjectStore,
    _marker: PhantomData<B>,
}

impl<B: DocumentBucket> DocumentStore<B> {
    /// Creates a new document store for the specified bucket type.
    pub async fn new(jetstream: &jetstream::Context) -> Result<Self> {
        let inner = ObjectStore::new(jetstream, B::NAME, B::MAX_AGE).await?;
        Ok(Self {
            inner,
            _marker: PhantomData,
        })
    }

    /// Streams data to the store while computing SHA-256 hash on-the-fly.
    ///
    /// This method does not buffer the entire content in memory, making it
    /// suitable for large file uploads.
    pub async fn put<R>(&self, key: &DocumentKey, reader: R) -> Result<PutResult>
    where
        R: tokio::io::AsyncRead + Unpin,
    {
        self.inner.put(&key.to_string(), reader).await
    }

    /// Gets an object from the store as a stream.
    ///
    /// Returns `None` if the object doesn't exist.
    /// The returned reader implements `AsyncRead` for streaming the content.
    pub async fn get(&self, key: &DocumentKey) -> Result<Option<GetResult>> {
        self.inner.get(&key.to_string()).await
    }

    /// Deletes an object from the store using a document key.
    pub async fn delete(&self, key: &DocumentKey) -> Result<()> {
        self.inner.delete(&key.to_string()).await
    }

    /// Checks if an object exists using a document key.
    pub async fn exists(&self, key: &DocumentKey) -> Result<bool> {
        self.inner.exists(&key.to_string()).await
    }

    /// Returns the bucket name for this store.
    #[inline]
    pub fn bucket(&self) -> &'static str {
        B::NAME
    }
}
