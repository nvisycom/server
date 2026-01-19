//! Data output trait for writing to storage backends.

use async_trait::async_trait;
use bytes::Bytes;
use futures::Stream;

use crate::error::DataResult;

/// Context for data output operations.
#[derive(Debug, Clone, Default)]
pub struct OutputContext {
    /// The bucket or container name (for object storage).
    pub bucket: Option<String>,
    /// Content type for the data being written.
    pub content_type: Option<String>,
    /// Additional options as key-value pairs.
    pub options: std::collections::HashMap<String, String>,
}

impl OutputContext {
    /// Creates a new empty context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the bucket/container.
    pub fn with_bucket(mut self, bucket: impl Into<String>) -> Self {
        self.bucket = Some(bucket.into());
        self
    }

    /// Sets the content type.
    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }

    /// Adds an option.
    pub fn with_option(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.options.insert(key.into(), value.into());
        self
    }
}

/// Trait for writing data to storage backends.
#[async_trait]
pub trait DataOutput: Send + Sync {
    /// Writes data to the given path.
    async fn write(&self, ctx: &OutputContext, path: &str, data: Bytes) -> DataResult<()>;

    /// Writes data from a stream to the given path.
    async fn write_stream(
        &self,
        ctx: &OutputContext,
        path: &str,
        stream: Box<dyn Stream<Item = DataResult<Bytes>> + Send + Unpin>,
    ) -> DataResult<()>;

    /// Deletes the data at the given path.
    async fn delete(&self, ctx: &OutputContext, path: &str) -> DataResult<()>;
}
