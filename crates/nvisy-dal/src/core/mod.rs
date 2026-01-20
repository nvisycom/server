//! Core types and traits for data operations.

mod context;
mod stream;

pub use context::Context;
pub use stream::{InputStream, ItemSink, ItemStream, OutputStream};

use crate::Result;
use crate::datatype::DataType;

/// Trait for reading data from a source.
///
/// Implementations provide streaming access to data with optional pagination.
#[async_trait::async_trait]
pub trait DataInput<T: DataType>: Send + Sync {
    /// Reads items from the source.
    ///
    /// Returns an input stream containing items and an optional cursor
    /// for pagination.
    async fn read(&self, ctx: &Context) -> Result<InputStream<'static, T>>;
}

/// Trait for writing data to a sink.
///
/// Implementations accept batches of items for writing.
#[async_trait::async_trait]
pub trait DataOutput<T: DataType>: Send + Sync {
    /// Writes a batch of items to the sink.
    async fn write(&self, ctx: &Context, items: Vec<T>) -> Result<()>;
}
