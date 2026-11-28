//! The S3-compatible blob store.

use futures::TryStreamExt;
use nvisy_core::{Error as CoreError, ErrorKind};
use nvisy_object_store::client::ObjectStoreClient;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio_util::io::{ReaderStream, StreamReader};

use crate::error::{Error, Result};
use crate::key::ObjectKey;

/// Tracing target for blob-store operations.
const TRACING_TARGET: &str = "nvisy_s3::store";

/// Size of each read from an upload's reader.
const READ_CHUNK_SIZE: usize = 64 * 1024;

/// A handle to a stored object's content and size.
///
/// `into_reader` yields an [`AsyncRead`] over the object's bytes; `size` is the
/// object's length in bytes as reported by storage.
pub struct GetObject {
    reader: Box<dyn AsyncRead + Send + Unpin>,
    size: u64,
}

impl GetObject {
    /// The object size in bytes.
    #[must_use]
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Consumes the handle, returning an [`AsyncRead`] over the object's bytes.
    #[must_use]
    pub fn into_reader(self) -> Box<dyn AsyncRead + Send + Unpin> {
        self.reader
    }
}

/// A first-party blob store backed by an S3-compatible service.
///
/// One S3 bucket holds every logical [`Bucket`]; each object is addressed by
/// `"{bucket.prefix()}/{key}"`, and the bucket is derived from the key's
/// [`ObjectKey::BUCKET`]. Cloneable and cheap to pass around: it wraps an
/// `Arc`-backed [`ObjectStoreClient`].
///
/// [`Bucket`]: crate::Bucket
#[derive(Clone)]
pub struct BlobStore {
    client: ObjectStoreClient,
}

impl BlobStore {
    /// Wraps a client already scoped to the configured bucket.
    pub(crate) fn new(client: ObjectStoreClient) -> Self {
        Self { client }
    }

    /// The full S3 object key for `key`, namespaced under its store's prefix.
    fn object_key<K: ObjectKey>(key: &K) -> String {
        format!("{}/{}", K::BUCKET.prefix(), key)
    }

    /// Streams `reader` into `key`'s store, returning the number of bytes written.
    ///
    /// The target store is [`K::BUCKET`], so a key can only be written to its own
    /// store. Small content is written with a single put and larger content as a
    /// multipart upload, so an object of unknown length streams without being
    /// buffered whole in memory; a failed upload is aborted so nothing partial
    /// lingers.
    ///
    /// # Errors
    ///
    /// `Operation` if reading from `reader` fails or the store rejects the upload.
    ///
    /// [`K::BUCKET`]: ObjectKey::BUCKET
    pub async fn put<K, R>(&self, key: &K, reader: R) -> Result<u64>
    where
        K: ObjectKey,
        R: AsyncRead + Unpin + Send,
    {
        let object_key = Self::object_key(key);
        tracing::debug!(target: TRACING_TARGET, key = %object_key, "Uploading object");

        let body = ReaderStream::with_capacity(reader, READ_CHUNK_SIZE)
            .map_err(|err| CoreError::runtime("failed to read the upload body").with_source(err));
        // Box the upload future so it is heap-allocated rather than inlined into
        // `put`'s own future: with a deeply nested reader type the combined future
        // is otherwise large enough to overflow a worker thread's stack.
        let size = Box::pin(self.client.put_stream(&object_key, None, body))
            .await
            .map_err(|err| Error::operation("put", err))?;

        tracing::debug!(target: TRACING_TARGET, key = %object_key, size, "Object uploaded");
        Ok(size)
    }

    /// Fetches an object as a stream, or `None` if it does not exist.
    ///
    /// # Errors
    ///
    /// `Operation` if the fetch fails for any reason other than a missing object.
    pub async fn get<K: ObjectKey>(&self, key: &K) -> Result<Option<GetObject>> {
        let object_key = Self::object_key(key);
        tracing::debug!(target: TRACING_TARGET, key = %object_key, "Fetching object");

        match self.client.get_stream(&object_key).await {
            Ok(object) => Ok(Some(GetObject {
                reader: Box::new(StreamReader::new(
                    object.body.map_err(std::io::Error::other),
                )),
                size: object.size,
            })),
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
            Err(err) => Err(Error::operation("get", err)),
        }
    }

    /// Fetches an object's whole content, or `None` if it does not exist.
    ///
    /// # Errors
    ///
    /// `Operation` if the fetch fails for any reason other than a missing object,
    /// or reading the body fails.
    pub async fn get_bytes<K: ObjectKey>(&self, key: &K) -> Result<Option<Vec<u8>>> {
        let Some(object) = self.get(key).await? else {
            return Ok(None);
        };
        let mut bytes = Vec::with_capacity(usize::try_from(object.size).unwrap_or(0));
        object
            .into_reader()
            .read_to_end(&mut bytes)
            .await
            .map_err(|err| {
                let source = CoreError::runtime("failed to read the object body").with_source(err);
                Error::operation("read", source)
            })?;
        Ok(Some(bytes))
    }

    /// Deletes an object. Idempotent: deleting a missing object succeeds.
    ///
    /// # Errors
    ///
    /// `Operation` if the store rejects the delete request.
    pub async fn delete<K: ObjectKey>(&self, key: &K) -> Result<()> {
        let object_key = Self::object_key(key);
        tracing::debug!(target: TRACING_TARGET, key = %object_key, "Deleting object");

        self.client
            .delete(&object_key)
            .await
            .map_err(|err| Error::operation("delete", err))
    }

    /// Probes the store's liveness with a bounded top-level listing of the
    /// configured bucket.
    ///
    /// Succeeds when the endpoint is reachable and the bucket exists and is
    /// listable with the current credentials, the same preconditions every
    /// object operation needs, so it doubles as a readiness check.
    ///
    /// # Errors
    ///
    /// `Operation` if the endpoint is unreachable, the credentials are rejected,
    /// or the bucket is missing or inaccessible.
    pub async fn ping(&self) -> Result<()> {
        self.client
            .verify_reachable()
            .await
            .map_err(|err| Error::operation("ping", err))
    }
}
