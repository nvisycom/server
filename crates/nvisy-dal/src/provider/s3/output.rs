//! S3 DataOutput implementation.

use async_trait::async_trait;

use super::S3Provider;
use crate::core::DataOutput;
use crate::datatype::Blob;
use crate::error::{Error, Result};

#[async_trait]
impl DataOutput for S3Provider {
    type Item = Blob;

    async fn write(&self, items: Vec<Blob>) -> Result<()> {
        for blob in items {
            self.operator
                .write(&blob.path, blob.data)
                .await
                .map_err(|e| Error::provider(e.to_string()))?;
        }
        Ok(())
    }
}
