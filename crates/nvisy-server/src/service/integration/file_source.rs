//! Provider-neutral file transfer for connection sync, and the two source
//! implementations behind it.
//!
//! The sync engine streams bytes in and out without knowing whether the
//! connection is backed by an object store (S3/Azure/GCS) or a file service
//! (Google Drive, Dropbox, ...). [`FileSource`] is the surface both families
//! share: stream one entry's bytes, and upload a stream. [`ObjectStoreSource`]
//! and [`FileServiceSource`] implement it by delegating to their respective
//! clients. Listing is object-store-only (a file service imports through the
//! frontend picker, which hands the backend explicit ids), so it is not part of
//! the trait — [`ObjectStoreSource::list`] exposes it directly for the
//! whole-listing import.

use bytes::Bytes;
use futures::TryStreamExt;
use futures::stream::BoxStream;
use nvisy_file_service::client::FileServiceClient;
use nvisy_object_store::Error as ObjectError;
use nvisy_object_store::client::ObjectStoreClient;

use crate::response::Result;

/// One entry to transfer, addressed by the provider-specific `key` the transfer
/// methods accept.
#[derive(Debug, Clone)]
pub struct SourceEntry {
    /// The key that addresses this entry for `get_stream`. For an object store it
    /// is the object path; for a file service it is the provider's file id.
    pub key: String,
    /// The entry's human-readable name, used to derive an imported file's
    /// display name and extension. For an object store it is the object path,
    /// which already carries the name.
    pub name: String,
}

/// A byte stream, the shape both directions of a transfer move data in.
pub type ByteStream = BoxStream<'static, Result<Bytes>>;

/// Provider-neutral read/write access to a connection's files.
///
/// Implemented by [`ObjectStoreSource`] and [`FileServiceSource`], each
/// delegating to its client. The sync engine holds it as `Arc<dyn FileSource>`.
#[async_trait::async_trait]
pub trait FileSource: Send + Sync {
    /// Streams one entry's bytes without buffering the whole entry in memory.
    async fn get_stream(&self, key: &str) -> Result<ByteStream>;

    /// Uploads a file to `upload.key`, streaming its body to the provider.
    async fn put_stream(&self, upload: FileUpload<'_>) -> Result<()>;
}

/// A streamed upload to a [`FileSource`]: the destination key, content type,
/// exact byte length, and body.
///
/// `content_length` must equal the bytes `body` yields; a file service that
/// needs the length up front (Dropbox) sets it as the request `Content-Length`,
/// while an object store streams via multipart and ignores it.
#[must_use]
pub struct FileUpload<'a> {
    /// The destination key: an object path, or a file service's new file name.
    pub key: &'a str,
    /// The MIME type of the content.
    pub content_type: &'a str,
    /// The exact number of bytes `body` yields.
    pub content_length: u64,
    /// The file's bytes, streamed to the provider.
    pub body: ByteStream,
}

/// A [`FileSource`] over an object store (S3/Azure/GCS).
///
/// Object stores address entries by path, so a listed entry's key and name are
/// both the object path.
pub struct ObjectStoreSource(pub ObjectStoreClient);

impl ObjectStoreSource {
    /// Lists every object under the connection's root, for a whole-listing
    /// import. Object-store only: file services import through the picker, so
    /// this is inherent rather than part of [`FileSource`].
    pub async fn list(&self) -> Result<Vec<SourceEntry>> {
        let objects = self.0.list("").await?;
        Ok(objects
            .into_iter()
            .map(|meta| {
                let key = meta.location.as_ref().to_owned();
                SourceEntry {
                    name: key.clone(),
                    key,
                }
            })
            .collect())
    }
}

#[async_trait::async_trait]
impl FileSource for ObjectStoreSource {
    async fn get_stream(&self, key: &str) -> Result<ByteStream> {
        let stream = self.0.get_stream(key).await?;
        Ok(Box::pin(stream.map_err(Into::into)))
    }

    async fn put_stream(&self, upload: FileUpload<'_>) -> Result<()> {
        // Object stores stream via multipart, which frames its own parts, so the
        // known length is unused here.
        let FileUpload {
            key,
            content_type,
            body,
            ..
        } = upload;
        // The multipart API wants an object-store-error stream; the body carries
        // the server error type, so map each item back at the boundary.
        let body = body.map_err(|err| ObjectError::runtime(err, "source-read"));
        self.0.put_multipart(key, Some(content_type), body).await?;
        Ok(())
    }
}

/// A [`FileSource`] over a file service (Google Drive, Dropbox, ...).
///
/// A file service addresses entries by an opaque id (the picker hands those ids
/// to the backend), so `get_stream`'s key is a file id and `put_stream`'s key is
/// the desired name of the new file.
pub struct FileServiceSource(pub Box<dyn FileServiceClient>);

#[async_trait::async_trait]
impl FileSource for FileServiceSource {
    async fn get_stream(&self, key: &str) -> Result<ByteStream> {
        let stream = self.0.get_stream(key).await?;
        Ok(Box::pin(stream.map_err(Into::into)))
    }

    async fn put_stream(&self, upload: FileUpload<'_>) -> Result<()> {
        // A file service creates a new file named by `key`; the caller passes the
        // desired file name as the key for an export.
        let FileUpload {
            key,
            content_type,
            content_length,
            body,
        } = upload;
        let body = body.map_err(|err| {
            nvisy_file_service::Error::runtime(format!("export stream failed: {err}"))
        });
        self.0
            .put_stream(nvisy_file_service::client::FileUpload {
                name: key,
                content_type,
                content_length,
                body: Box::pin(body),
            })
            .await?;
        Ok(())
    }
}
