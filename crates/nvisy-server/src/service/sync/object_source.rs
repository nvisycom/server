//! [`FileSource`] over an object store (S3/Azure/GCS).
//!
//! Delegates to [`ObjectStoreClient`], mapping between the object-store byte
//! streams and the provider-neutral [`ByteStream`] the sync engine uses. Object
//! stores address entries by path, so a listed entry's key and name are both the
//! object path.

use futures::TryStreamExt;
use nvisy_object_store::Error as ObjectError;
use nvisy_object_store::client::ObjectStoreClient;

use super::file_source::{ByteStream, FileSource, SourceEntry};
use crate::handler::Result;

/// Wraps an [`ObjectStoreClient`] as a [`FileSource`].
pub struct ObjectStoreSource(pub ObjectStoreClient);

#[async_trait::async_trait]
impl FileSource for ObjectStoreSource {
    async fn list(&self) -> Result<Vec<SourceEntry>> {
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

    async fn get_stream(&self, key: &str) -> Result<ByteStream> {
        let stream = self.0.get_stream(key).await?;
        Ok(Box::pin(stream.map_err(Into::into)))
    }

    async fn put_stream(&self, key: &str, content_type: &str, body: ByteStream) -> Result<()> {
        // The multipart API wants an object-store-error stream; the body carries
        // the server error type, so map each item back at the boundary.
        let body = body.map_err(|err| ObjectError::runtime(err, "source-read"));
        self.0.put_multipart(key, Some(content_type), body).await?;
        Ok(())
    }
}
