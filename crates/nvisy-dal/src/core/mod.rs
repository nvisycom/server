//! Core types and traits for data operations.

mod contexts;
mod datatypes;
mod params;
mod streams;

pub use contexts::{AnyContext, ObjectContext, RelationalContext, VectorContext};
pub use datatypes::{
    AnyDataValue, DataType, Document, Edge, Embedding, Graph, Message, Metadata, Node, Object,
    Record,
};
pub use nvisy_core::Provider;
pub use params::{DistanceMetric, ObjectParams, RelationalParams, VectorParams};
pub use streams::{InputStream, ItemSink, ItemStream, OutputStream};

use crate::Result;

/// Trait for reading data from a source.
///
/// Implementations provide streaming access to data with optional pagination.
#[async_trait::async_trait]
pub trait DataInput: Send + Sync {
    /// The item type produced by this provider.
    type Item;
    /// The context type for read operations.
    type Context;

    /// Reads items from the source.
    ///
    /// Returns an input stream containing items.
    async fn read(&self, ctx: &Self::Context) -> Result<InputStream<Self::Item>>;
}

/// Trait for writing data to a sink.
///
/// Implementations accept batches of items for writing.
#[async_trait::async_trait]
pub trait DataOutput: Send + Sync {
    /// The item type accepted by this provider.
    type Item;

    /// Writes a batch of items to the sink.
    async fn write(&self, items: Vec<Self::Item>) -> Result<()>;
}
