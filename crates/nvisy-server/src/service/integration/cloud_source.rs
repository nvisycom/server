//! [`FileSource`] over a cloud file service (Google Drive, ...).
//!
//! Wraps a connected [`FileServiceClient`], mapping its byte streams onto the
//! provider-neutral [`FileSource`] the sync engine drives. A file service
//! addresses entries by an opaque id (the picker hands those ids to the backend),
//! so `get_stream`'s key is a file id.

use futures::TryStreamExt;
use nvisy_file_service::client::FileServiceClient;

use super::file_source::{ByteStream, FileSource};
use crate::handler::Result;

/// Wraps a connected [`FileServiceClient`] as a [`FileSource`].
pub struct CloudFileSource(pub Box<dyn FileServiceClient>);

#[async_trait::async_trait]
impl FileSource for CloudFileSource {
    async fn get_stream(&self, key: &str) -> Result<ByteStream> {
        let stream = self.0.get_stream(key).await?;
        Ok(Box::pin(stream.map_err(Into::into)))
    }

    async fn put_stream(&self, key: &str, content_type: &str, body: ByteStream) -> Result<()> {
        // A file service creates a new file named by `key`; the caller passes the
        // desired file name as the key for an export.
        let body = body.map_err(|err| {
            nvisy_file_service::Error::runtime(format!("export stream failed: {err}"))
        });
        self.0.put_stream(key, content_type, Box::pin(body)).await?;
        Ok(())
    }
}
