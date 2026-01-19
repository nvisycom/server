//! Data input trait for reading from storage backends.

use async_trait::async_trait;
use bytes::Bytes;
use futures::Stream;

use crate::error::DataResult;

/// Context for data input operations.
#[derive(Debug, Clone, Default)]
pub struct InputContext {
    /// The bucket or container name (for object storage).
    pub bucket: Option<String>,
    /// Additional options as key-value pairs.
    pub options: std::collections::HashMap<String, String>,
}

impl InputContext {
    /// Creates a new empty context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the bucket/container.
    pub fn with_bucket(mut self, bucket: impl Into<String>) -> Self {
        self.bucket = Some(bucket.into());
        self
    }

    /// Adds an option.
    pub fn with_option(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.options.insert(key.into(), value.into());
        self
    }
}

/// Trait for reading data from storage backends.
#[async_trait]
pub trait DataInput: Send + Sync {
    /// Reads the entire contents at the given path.
    async fn read(&self, ctx: &InputContext, path: &str) -> DataResult<Bytes>;

    /// Reads the contents as a stream of chunks.
    async fn read_stream(
        &self,
        ctx: &InputContext,
        path: &str,
    ) -> DataResult<Box<dyn Stream<Item = DataResult<Bytes>> + Send + Unpin>>;

    /// Checks if a path exists.
    async fn exists(&self, ctx: &InputContext, path: &str) -> DataResult<bool>;

    /// Lists paths under the given prefix.
    async fn list(&self, ctx: &InputContext, prefix: &str) -> DataResult<Vec<String>>;
}
