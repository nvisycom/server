//! Unified object-store client backed by [`object_store::ObjectStore`].
//!
//! [`ObjectStoreClient`] is a thin, cloneable wrapper around
//! `Arc<dyn ObjectStore>` that provides convenience methods for the most
//! common operations. Every public method is instrumented with
//! [`tracing`] for observability.

use std::ops::Range;
use std::sync::Arc;

use bytes::Bytes;
use futures::stream::BoxStream;
use futures::{Stream, StreamExt, TryStreamExt};
use nvisy_core::{Error, ErrorKind};
use object_store::buffered::BufWriter;
use object_store::path::Path;
use object_store::{
    Attribute, Attributes, ObjectMeta, ObjectStore, ObjectStoreExt, PutMode, PutOptions, PutPayload,
};
use tokio::io::AsyncWriteExt;

mod get_output;
mod get_stream;
mod put_output;
mod service;

pub use get_output::GetOutput;
pub use get_stream::GetStream;
pub use put_output::PutOutput;
pub use service::ExternalObjectStore;

/// Maximum number of in-flight multipart part uploads, bounding memory and
/// concurrent requests during a [`put_stream`](ObjectStoreClient::put_stream).
const MULTIPART_MAX_CONCURRENCY: usize = 8;

/// Size of each multipart part, and the most a [`put_stream`] buffers before
/// switching from a single put to a multipart upload. At 8 MiB, S3's limit of
/// 10,000 parts allows uploads of up to about 78 GiB.
///
/// [`put_stream`]: ObjectStoreClient::put_stream
const PART_SIZE: usize = 8 * 1024 * 1024;

/// Parses a caller-supplied key into an object-store [`Path`], surfacing a
/// malformed key as an error rather than silently normalizing it.
fn parse_key(key: &str) -> Result<Path, Error> {
    Path::parse(key)
        .map_err(|e| Error::invalid(format!("invalid object key {key:?}")).with_source(e))
}

/// Classifies an [`object_store::Error`] into the shared error kinds.
fn store_error(err: object_store::Error) -> Error {
    use object_store::Error as O;
    let kind = match &err {
        O::NotFound { .. } => ErrorKind::NotFound,
        O::AlreadyExists { .. } => ErrorKind::AlreadyExists,
        O::Precondition { .. } | O::NotModified { .. } => ErrorKind::Precondition,
        O::PermissionDenied { .. } => ErrorKind::PermissionDenied,
        O::Unauthenticated { .. } => ErrorKind::Unauthenticated,
        O::NotSupported { .. } | O::NotImplemented { .. } => ErrorKind::NotSupported,
        _ => ErrorKind::Runtime,
    };
    Error::new(kind, err.to_string()).with_source(err)
}

/// Classifies an I/O error from a [`BufWriter`], recovering the
/// [`object_store::Error`] it wraps when there is one.
fn io_store_error(err: std::io::Error) -> Error {
    let message = err.to_string();
    let Some(inner) = err.into_inner() else {
        return Error::runtime(message);
    };
    match inner.downcast::<object_store::Error>() {
        Ok(store) => store_error(*store),
        Err(other) => Error::runtime(message).with_source(other),
    }
}

/// Writes `bytes` in part-sized slices, so one large chunk waits for upload
/// capacity per part rather than queueing every part at once.
async fn write_in_parts(writer: &mut BufWriter, mut bytes: Bytes) -> Result<(), Error> {
    while !bytes.is_empty() {
        let part = bytes.split_to(bytes.len().min(PART_SIZE));
        writer.put(part).await.map_err(store_error)?;
    }
    Ok(())
}

/// Cloneable handle to any [`ObjectStore`] backend (S3, Azure, GCS, ...).
///
/// All methods accept human-readable string keys and convert them to
/// [`object_store::path::Path`] internally.
#[derive(Clone, Debug)]
pub struct ObjectStoreClient(pub Arc<dyn ObjectStore>);

impl ObjectStoreClient {
    /// Wrap a concrete [`ObjectStore`] implementation.
    pub fn new(store: impl ObjectStore) -> Self {
        Self(Arc::new(store))
    }

    /// Verify that the backing store is reachable and the credentials work.
    ///
    /// Requests a single listing page: it succeeds for any reachable
    /// bucket/container (including an empty one) and surfaces authorization
    /// failures (`PermissionDenied`/`Unauthenticated`) as errors, so callers
    /// can distinguish a bad connection from bad credentials. Unlike a HEAD on
    /// a fabricated key, it does not depend on a probe object existing.
    ///
    /// # Errors
    ///
    /// A storage error if the store is unreachable or the credentials are
    /// rejected (`PermissionDenied`/`Unauthenticated`).
    #[tracing::instrument(name = "object.verify", skip(self))]
    pub async fn verify_reachable(&self) -> Result<(), Error> {
        // The listing stream fetches pages lazily, so reading one entry sends one
        // request; an empty bucket ends the stream without an error.
        match self.0.list(None).next().await {
            Some(Err(err)) => Err(store_error(err)),
            Some(Ok(_)) | None => Ok(()),
        }
    }

    /// List object keys under `prefix`.
    ///
    /// Returns all matching keys in a single `Vec`. For lazy iteration,
    /// use [`list_stream`] instead.
    ///
    /// [`list_stream`]: Self::list_stream
    ///
    /// # Errors
    ///
    /// A storage error if the listing request fails.
    #[tracing::instrument(name = "object.list", skip(self), fields(prefix = %prefix))]
    pub async fn list(&self, prefix: &str) -> Result<Vec<ObjectMeta>, Error> {
        let prefix = if prefix.is_empty() {
            None
        } else {
            Some(Path::from(prefix))
        };
        self.0
            .list(prefix.as_ref())
            .try_collect()
            .await
            .map_err(store_error)
    }

    /// Lazily stream object metadata under `prefix`.
    #[tracing::instrument(name = "object.list_stream", skip(self), fields(prefix = %prefix))]
    pub fn list_stream(&self, prefix: &str) -> BoxStream<'_, Result<ObjectMeta, Error>> {
        let prefix = if prefix.is_empty() {
            None
        } else {
            Some(Path::from(prefix))
        };
        Box::pin(self.0.list(prefix.as_ref()).map_err(store_error))
    }

    /// Retrieve the raw bytes, content-type, and metadata stored at `key`.
    ///
    /// # Errors
    ///
    /// - A `Runtime` error if `key` is not a valid object-store path.
    /// - A storage error (including `NotFound`) if the object cannot be fetched
    ///   or its body cannot be read.
    #[tracing::instrument(name = "object.get", skip(self), fields(key = %key))]
    pub async fn get(&self, key: &str) -> Result<GetOutput, Error> {
        let path = parse_key(key)?;
        let result = self.0.get(&path).await.map_err(store_error)?;
        let meta = result.meta.clone();
        let content_type = result
            .attributes
            .get(&Attribute::ContentType)
            .map(|v| v.to_string());
        let data = result.bytes().await.map_err(store_error)?;
        Ok(GetOutput {
            data,
            content_type,
            meta,
        })
    }

    /// Opens the object at `key` as a stream of chunks, with its size, without
    /// buffering the whole object in memory.
    ///
    /// # Errors
    ///
    /// - An `Invalid` error if `key` is not a valid object-store path.
    /// - A storage error (including `NotFound`) if the object cannot be opened.
    ///   Errors reading individual chunks are yielded by the returned stream.
    #[tracing::instrument(name = "object.get_stream", skip(self), fields(key = %key))]
    pub async fn get_stream(&self, key: &str) -> Result<GetStream, Error> {
        let path = parse_key(key)?;
        let result = self.0.get(&path).await.map_err(store_error)?;
        Ok(GetStream {
            size: result.meta.size,
            body: Box::pin(result.into_stream().map_err(store_error)),
        })
    }

    /// Upload `data` to `key`, optionally setting the content-type.
    ///
    /// # Errors
    ///
    /// Propagates any error from [`put_opts`](Self::put_opts).
    pub async fn put(
        &self,
        key: &str,
        data: Bytes,
        content_type: Option<&str>,
    ) -> Result<PutOutput, Error> {
        self.put_opts(key, data, PutMode::Overwrite, content_type)
            .await
    }

    /// Upload `data` to `key` with the specified [`PutMode`].
    ///
    /// # Errors
    ///
    /// - A `Runtime` error if `key` is not a valid object-store path.
    /// - A storage error if the upload fails (including a precondition failure
    ///   under a conditional [`PutMode`]).
    #[tracing::instrument(name = "object.put_opts", skip(self, data), fields(key = %key, size = data.len()))]
    pub async fn put_opts(
        &self,
        key: &str,
        data: Bytes,
        mode: PutMode,
        content_type: Option<&str>,
    ) -> Result<PutOutput, Error> {
        let path = parse_key(key)?;
        let payload = PutPayload::from(data);
        let mut opts = PutOptions {
            mode,
            ..Default::default()
        };
        if let Some(ct) = content_type {
            opts.attributes
                .insert(Attribute::ContentType, ct.to_string().into());
        }
        let result = self
            .0
            .put_opts(&path, payload, opts)
            .await
            .map_err(store_error)?;
        Ok(result.into())
    }

    /// Get object metadata without downloading the body.
    ///
    /// # Errors
    ///
    /// - A `Runtime` error if `key` is not a valid object-store path.
    /// - A storage error (including `NotFound`) if the metadata cannot be fetched.
    #[tracing::instrument(name = "object.head", skip(self), fields(key = %key))]
    pub async fn head(&self, key: &str) -> Result<ObjectMeta, Error> {
        let path = parse_key(key)?;
        self.0.head(&path).await.map_err(store_error)
    }

    /// Delete the object at `key`.
    ///
    /// # Errors
    ///
    /// - A `Runtime` error if `key` is not a valid object-store path.
    /// - A storage error if the delete request fails.
    #[tracing::instrument(name = "object.delete", skip(self), fields(key = %key))]
    pub async fn delete(&self, key: &str) -> Result<(), Error> {
        let path = parse_key(key)?;
        self.0.delete(&path).await.map_err(store_error)
    }

    /// Copy an object from `src` to `dst` within the same store.
    ///
    /// # Errors
    ///
    /// - A `Runtime` error if either `src` or `dst` is not a valid object-store
    ///   path.
    /// - A storage error (including `NotFound` for a missing source) if the copy
    ///   fails.
    #[tracing::instrument(name = "object.copy", skip(self), fields(src = %src, dst = %dst))]
    pub async fn copy(&self, src: &str, dst: &str) -> Result<(), Error> {
        let from = parse_key(src)?;
        let to = parse_key(dst)?;
        self.0.copy(&from, &to).await.map_err(store_error)
    }

    /// Whether an object exists at `key`.
    ///
    /// # Errors
    ///
    /// - A `Runtime` error if `key` is not a valid object-store path.
    /// - A storage error other than `NotFound` (which resolves to `Ok(false)`)
    ///   if the existence check fails.
    #[tracing::instrument(name = "object.exists", skip(self), fields(key = %key))]
    pub async fn exists(&self, key: &str) -> Result<bool, Error> {
        match self.head(key).await {
            Ok(_) => Ok(true),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Read a byte range of the object at `key` without downloading the whole
    /// body.
    ///
    /// # Errors
    ///
    /// - A `Runtime` error if `key` is not a valid object-store path.
    /// - A storage error (including `NotFound`) if the range cannot be read.
    #[tracing::instrument(name = "object.get_range", skip(self), fields(key = %key))]
    pub async fn get_range(&self, key: &str, range: Range<u64>) -> Result<Bytes, Error> {
        let path = parse_key(key)?;
        self.0.get_range(&path, range).await.map_err(store_error)
    }

    /// Streams `stream` into `key`, optionally setting the content type, and
    /// returns the number of bytes written.
    ///
    /// Content up to one part (8 MiB) is buffered and written with a single put;
    /// anything larger streams as a multipart upload with at most 8 parts in
    /// flight, so the whole object is never held in memory. If the stream yields
    /// an error, or an upload fails, the upload is aborted so no partial object
    /// or orphaned parts are left behind.
    ///
    /// # Errors
    ///
    /// - An `Invalid` error if `key` is not a valid object-store path.
    /// - The stream's own error if it yields one (the upload is aborted first).
    /// - A storage error if a put, a part, or completing the upload fails.
    #[tracing::instrument(name = "object.put_stream", skip(self, stream), fields(key = %key))]
    pub async fn put_stream<S>(
        &self,
        key: &str,
        content_type: Option<&str>,
        mut stream: S,
    ) -> Result<u64, Error>
    where
        S: Stream<Item = Result<Bytes, Error>> + Unpin + Send,
    {
        let path = parse_key(key)?;
        let mut attributes = Attributes::new();
        if let Some(content_type) = content_type {
            attributes.insert(Attribute::ContentType, content_type.to_owned().into());
        }
        let mut writer = BufWriter::with_capacity(Arc::clone(&self.0), path, PART_SIZE)
            .with_max_concurrency(MULTIPART_MAX_CONCURRENCY)
            .with_attributes(attributes);

        let mut size: u64 = 0;
        while let Some(chunk) = stream.next().await {
            let written = match chunk {
                Ok(bytes) => {
                    size += u64::try_from(bytes.len()).unwrap_or(u64::MAX);
                    write_in_parts(&mut writer, bytes).await
                }
                Err(err) => Err(err),
            };
            if let Err(err) = written {
                // Abort so the backend keeps no partial upload.
                if let Err(abort) = writer.abort().await {
                    tracing::warn!(key = %key, error = %abort, "Failed to abort an upload");
                }
                return Err(err);
            }
        }

        // Completes the single put or the multipart upload; a failed complete
        // aborts the multipart upload itself.
        writer.shutdown().await.map_err(io_store_error)?;
        Ok(size)
    }
}

#[cfg(test)]
mod tests {
    use object_store::memory::InMemory;

    use super::*;

    fn test_client() -> ObjectStoreClient {
        ObjectStoreClient::new(InMemory::new())
    }

    #[tokio::test]
    async fn put_and_get() {
        let client = test_client();
        let data = Bytes::from("hello world");
        client
            .put("test.txt", data.clone(), Some("text/plain"))
            .await
            .unwrap();

        let result = client.get("test.txt").await.unwrap();
        assert_eq!(result.data, data);
        assert_eq!(result.content_type.as_deref(), Some("text/plain"));
    }

    #[tokio::test]
    async fn verify_reachable() {
        let client = test_client();
        client.verify_reachable().await.unwrap();
    }

    #[tokio::test]
    async fn put_stream_splits_one_large_chunk_into_parts() {
        // One stream item spanning more parts than may be in flight: it is
        // written a part at a time, so the upload stays bounded. The written
        // size and the reassembled object must both match the input.
        let client = test_client();
        let size = PART_SIZE * (MULTIPART_MAX_CONCURRENCY + 3) + 123;
        let data = Bytes::from(vec![7u8; size]);
        let stream = futures::stream::once({
            let data = data.clone();
            async move { Ok(data) }
        });

        let written = client
            .put_stream(
                "big.bin",
                Some("application/octet-stream"),
                Box::pin(stream),
            )
            .await
            .unwrap();
        assert_eq!(written, u64::try_from(size).unwrap());

        let result = client.get("big.bin").await.unwrap();
        assert_eq!(result.data.len(), size);
        assert_eq!(result.data, data);
    }

    #[tokio::test]
    async fn exists() {
        let client = test_client();
        assert!(!client.exists("nope.bin").await.unwrap());
        client.put("yes.bin", Bytes::from("x"), None).await.unwrap();
        assert!(client.exists("yes.bin").await.unwrap());
    }

    #[tokio::test]
    async fn put_stream_writes_every_chunk() {
        let client = test_client();
        let chunks = vec![
            Ok(Bytes::from("hello ")),
            Ok(Bytes::from("multipart ")),
            Ok(Bytes::from("world")),
        ];
        client
            .put_stream("mp.txt", Some("text/plain"), futures::stream::iter(chunks))
            .await
            .unwrap();

        let result = client.get("mp.txt").await.unwrap();
        assert_eq!(result.data, Bytes::from("hello multipart world"));
        assert_eq!(result.content_type.as_deref(), Some("text/plain"));
    }

    #[tokio::test]
    async fn put_stream_aborts_on_stream_error() {
        let client = test_client();
        let chunks = vec![
            Ok(Bytes::from("partial")),
            Err(Error::runtime("stream broke")),
        ];
        let err = client
            .put_stream("aborted.txt", None, futures::stream::iter(chunks))
            .await
            .unwrap_err();
        assert_eq!(err.to_string(), "stream broke");
        // The object must not have been committed.
        assert!(!client.exists("aborted.txt").await.unwrap());
    }

    #[tokio::test]
    async fn error_kind_mapping() {
        let client = test_client();
        let err = client.head("missing.bin").await.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::NotFound);
        assert!(!err.is_retryable());
    }
}
