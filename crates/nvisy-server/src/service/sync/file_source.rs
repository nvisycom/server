//! Provider-neutral file access for connection sync.
//!
//! The sync engine imports and exports objects without knowing whether the
//! connection is backed by an object store (S3/Azure/GCS) or a consumer file
//! service (Google Drive, Dropbox, ...). [`FileSource`] is the small surface it
//! needs: list the source, stream one entry's bytes, and upload a stream. Each
//! provider family implements it, and
//! [`connect_file_source`](super::service::ConnectionSyncService::connect_file_source)
//! turns a typed connection config into the right one.

use bytes::Bytes;
use futures::stream::BoxStream;

use crate::handler::Result;

/// One listed entry in a source, addressed by the provider-specific `key` the
/// other [`FileSource`] methods accept.
#[derive(Debug, Clone)]
pub struct SourceEntry {
    /// The key that addresses this entry for `get_stream`/`delete`. For an
    /// object store it is the object path; for a file service it is the
    /// provider's file identifier.
    pub key: String,
    /// The entry's human-readable name, used to derive an imported file's
    /// display name and extension. Defaults to the key for object stores, whose
    /// key already carries the path.
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
    /// Lists the entries available for import, already scoped to the
    /// connection's configured root.
    async fn list(&self) -> Result<Vec<SourceEntry>>;

    /// Streams one entry's bytes without buffering the whole entry in memory.
    async fn get_stream(&self, key: &str) -> Result<ByteStream>;

    /// Uploads `body` to `key`, streaming it to the provider.
    async fn put_stream(&self, key: &str, content_type: &str, body: ByteStream) -> Result<()>;
}
