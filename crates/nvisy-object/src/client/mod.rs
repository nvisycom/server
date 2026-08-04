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
use object_store::path::Path;
use object_store::{
    Attribute, Attributes, ObjectMeta, ObjectStore, ObjectStoreExt, PutMode, PutMultipartOptions,
    PutOptions, PutPayload, WriteMultipart,
};

use crate::error::{Error, ErrorKind};

mod get_output;
mod put_output;

pub use get_output::GetOutput;
pub use put_output::PutOutput;

/// Maximum number of in-flight multipart part uploads, bounding memory and
/// concurrent requests during a streaming [`put_multipart`](ObjectStoreClient::put_multipart).
const MULTIPART_MAX_CONCURRENCY: usize = 8;

/// Parses a caller-supplied key into an object-store [`Path`], surfacing a
/// malformed key as an error rather than silently normalizing it.
fn parse_key(key: &str) -> Result<Path, Error> {
    Path::parse(key).map_err(|e| Error::new(ErrorKind::Runtime, e, "object-store"))
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
    /// Performs a bounded top-level listing: it succeeds for any reachable
    /// bucket/container (including an empty one) and surfaces authorization
    /// failures (`PermissionDenied`/`Unauthenticated`) as errors, so callers
    /// can distinguish a bad connection from bad credentials. Unlike a HEAD on
    /// a fabricated key, it does not depend on a probe object existing.
    #[tracing::instrument(name = "object.verify", skip(self))]
    pub async fn verify_reachable(&self) -> Result<(), Error> {
        self.0
            .list_with_delimiter(None)
            .await
            .map(|_| ())
            .map_err(Error::from)
    }

    /// List object keys under `prefix`.
    ///
    /// Returns all matching keys in a single `Vec`. For lazy iteration,
    /// use [`list_stream`] instead.
    ///
    /// [`list_stream`]: Self::list_stream
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
            .map_err(Error::from)
    }

    /// Lazily stream object metadata under `prefix`.
    #[tracing::instrument(name = "object.list_stream", skip(self), fields(prefix = %prefix))]
    pub fn list_stream(&self, prefix: &str) -> BoxStream<'_, Result<ObjectMeta, Error>> {
        let prefix = if prefix.is_empty() {
            None
        } else {
            Some(Path::from(prefix))
        };
        Box::pin(self.0.list(prefix.as_ref()).map_err(Error::from))
    }

    /// Retrieve the raw bytes, content-type, and metadata stored at `key`.
    #[tracing::instrument(name = "object.get", skip(self), fields(key = %key))]
    pub async fn get(&self, key: &str) -> Result<GetOutput, Error> {
        let path = parse_key(key)?;
        let result = self.0.get(&path).await.map_err(Error::from)?;
        let meta = result.meta.clone();
        let content_type = result
            .attributes
            .get(&Attribute::ContentType)
            .map(|v| v.to_string());
        let data = result.bytes().await.map_err(Error::from)?;
        Ok(GetOutput {
            data,
            content_type,
            meta,
        })
    }

    /// Stream the body of the object at `key` as chunks, without buffering the
    /// whole object in memory.
    ///
    /// The read half of an object-to-elsewhere pipe (e.g. streaming an object
    /// into NATS); pair with a chunk-consuming sink.
    #[tracing::instrument(name = "object.get_stream", skip(self), fields(key = %key))]
    pub async fn get_stream(
        &self,
        key: &str,
    ) -> Result<BoxStream<'static, Result<Bytes, Error>>, Error> {
        let path = parse_key(key)?;
        let result = self.0.get(&path).await.map_err(Error::from)?;
        Ok(Box::pin(result.into_stream().map_err(Error::from)))
    }

    /// Upload `data` to `key`, optionally setting the content-type.
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
            .map_err(Error::from)?;
        Ok(result.into())
    }

    /// Get object metadata without downloading the body.
    #[tracing::instrument(name = "object.head", skip(self), fields(key = %key))]
    pub async fn head(&self, key: &str) -> Result<ObjectMeta, Error> {
        let path = parse_key(key)?;
        self.0.head(&path).await.map_err(Error::from)
    }

    /// Delete the object at `key`.
    #[tracing::instrument(name = "object.delete", skip(self), fields(key = %key))]
    pub async fn delete(&self, key: &str) -> Result<(), Error> {
        let path = parse_key(key)?;
        self.0.delete(&path).await.map_err(Error::from)
    }

    /// Copy an object from `src` to `dst` within the same store.
    #[tracing::instrument(name = "object.copy", skip(self), fields(src = %src, dst = %dst))]
    pub async fn copy(&self, src: &str, dst: &str) -> Result<(), Error> {
        let from = parse_key(src)?;
        let to = parse_key(dst)?;
        self.0.copy(&from, &to).await.map_err(Error::from)
    }

    /// Whether an object exists at `key`.
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
    #[tracing::instrument(name = "object.get_range", skip(self), fields(key = %key))]
    pub async fn get_range(&self, key: &str, range: Range<u64>) -> Result<Bytes, Error> {
        let path = parse_key(key)?;
        self.0.get_range(&path, range).await.map_err(Error::from)
    }

    /// Upload the contents of a byte stream to `key` using a multipart upload.
    ///
    /// Chunks are uploaded as they arrive, so the whole object never needs to
    /// be buffered in memory. If the stream yields an error, or any part fails,
    /// the multipart upload is aborted so no orphaned parts are left behind.
    #[tracing::instrument(name = "object.put_multipart", skip(self, stream), fields(key = %key))]
    pub async fn put_multipart<S>(
        &self,
        key: &str,
        content_type: Option<&str>,
        mut stream: S,
    ) -> Result<PutOutput, Error>
    where
        S: Stream<Item = Result<Bytes, Error>> + Unpin + Send,
    {
        let path = parse_key(key)?;
        let opts = match content_type {
            Some(ct) => {
                let mut attributes = Attributes::new();
                attributes.insert(Attribute::ContentType, ct.to_string().into());
                PutMultipartOptions {
                    attributes,
                    ..Default::default()
                }
            }
            None => PutMultipartOptions::default(),
        };

        let upload = self
            .0
            .put_multipart_opts(&path, opts)
            .await
            .map_err(Error::from)?;
        let mut writer = WriteMultipart::new(upload);

        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(bytes) => {
                    // Bound in-flight part uploads so a large body cannot spawn
                    // an unbounded number of concurrent requests.
                    if let Err(e) = writer.wait_for_capacity(MULTIPART_MAX_CONCURRENCY).await {
                        let _ = writer.abort().await;
                        return Err(Error::from(e));
                    }
                    writer.put(bytes);
                }
                Err(e) => {
                    // Abort so the backend does not retain orphaned parts.
                    let _ = writer.abort().await;
                    return Err(e);
                }
            }
        }

        match writer.finish().await {
            Ok(result) => Ok(result.into()),
            Err(e) => Err(Error::from(e)),
        }
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
    async fn exists() {
        let client = test_client();
        assert!(!client.exists("nope.bin").await.unwrap());
        client.put("yes.bin", Bytes::from("x"), None).await.unwrap();
        assert!(client.exists("yes.bin").await.unwrap());
    }

    #[tokio::test]
    async fn put_multipart_streams_chunks() {
        let client = test_client();
        let chunks = vec![
            Ok(Bytes::from("hello ")),
            Ok(Bytes::from("multipart ")),
            Ok(Bytes::from("world")),
        ];
        client
            .put_multipart("mp.txt", Some("text/plain"), futures::stream::iter(chunks))
            .await
            .unwrap();

        let result = client.get("mp.txt").await.unwrap();
        assert_eq!(result.data, Bytes::from("hello multipart world"));
        assert_eq!(result.content_type.as_deref(), Some("text/plain"));
    }

    #[tokio::test]
    async fn put_multipart_aborts_on_stream_error() {
        let client = test_client();
        let chunks = vec![
            Ok(Bytes::from("partial")),
            Err(Error::runtime("stream broke", "test")),
        ];
        let err = client
            .put_multipart("aborted.txt", None, futures::stream::iter(chunks))
            .await
            .unwrap_err();
        assert_eq!(err.to_string(), "[test] stream broke");
        // The object must not have been committed.
        assert!(!client.exists("aborted.txt").await.unwrap());
    }

    #[tokio::test]
    async fn error_kind_mapping() {
        let client = test_client();
        let err = client.head("missing.bin").await.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::NotFound);
        assert!(!err.is_retryable());
        assert!(err.retry_delay().is_none());
    }
}
