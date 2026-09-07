//! Provider-neutral file transfer for connection sync.
//!
//! The sync engine streams bytes in and out without knowing whether the
//! connection is backed by an object store (S3/Azure/GCS) or a consumer file
//! service (Google Drive, Dropbox, ...). [`FileSource`] is the surface both
//! families share: stream one entry's bytes, and upload a stream. Listing is
//! object-store-only (a file service imports through the frontend picker, which
//! hands the backend explicit ids), so it is not part of this trait —
//! [`ObjectStoreSource`](super::object_source::ObjectStoreSource) exposes it
//! directly for the whole-listing import.

use bytes::Bytes;
use futures::stream::BoxStream;

use crate::handler::Result;

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
/// Implemented for the object-store client (via delegation) and for each cloud
/// file-service client. The sync engine holds it as `Arc<dyn FileSource>`.
#[async_trait::async_trait]
pub trait FileSource: Send + Sync {
    /// Streams one entry's bytes without buffering the whole entry in memory.
    async fn get_stream(&self, key: &str) -> Result<ByteStream>;

    /// Uploads `body` to `key`, streaming it to the provider.
    async fn put_stream(&self, key: &str, content_type: &str, body: ByteStream) -> Result<()>;
}
