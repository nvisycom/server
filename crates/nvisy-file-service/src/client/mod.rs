//! The operational client surface the sync engine drives.
//!
//! Every provider's client implements [`FileServiceClient`]: list the files
//! available for import, stream one file's bytes, and upload a stream. It mirrors
//! the small surface the object-store client exposes, so the server can wrap
//! both behind one sync-side abstraction.

use bytes::Bytes;
use futures::stream::BoxStream;

use crate::error::Error;

/// One file listed in a service, addressed by the provider's file identifier.
#[derive(Debug, Clone)]
pub struct FileEntry {
    /// The provider's identifier for this file, used by `get_stream`. For a file
    /// service this is an opaque id, not a path.
    pub id: String,
    /// The file's human-readable name, used to derive the imported file's
    /// display name and extension.
    pub name: String,
}

/// A byte stream, the shape both directions of a transfer move data in.
pub type ByteStream = BoxStream<'static, Result<Bytes, Error>>;

/// Provider-neutral read/write access to a connected file service.
#[async_trait::async_trait]
pub trait FileServiceClient: Send + Sync {
    /// Verifies the connection is reachable with the current credentials,
    /// without transferring any file.
    async fn verify(&self) -> Result<(), Error>;

    /// Lists the files available for import, already scoped to the connection's
    /// configured root folder.
    async fn list(&self) -> Result<Vec<FileEntry>, Error>;

    /// Streams one file's bytes without buffering the whole file in memory.
    async fn get_stream(&self, id: &str) -> Result<ByteStream, Error>;

    /// Uploads `body` as a new file named `name`, streaming it to the provider.
    async fn put_stream(
        &self,
        name: &str,
        content_type: &str,
        body: ByteStream,
    ) -> Result<(), Error>;
}
