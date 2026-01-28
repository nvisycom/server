//! Core types and traits for data operations.

pub mod contexts;
pub mod datatypes;
pub mod params;
pub mod streams;

pub use nvisy_core::Provider;
use streams::InputStream;

use crate::Result;

/// Data paired with context for resumable streaming.
///
/// When reading from a data source, each item is paired with a context
/// that represents the state needed to resume reading after that item.
/// This allows for efficient recovery if streaming is interrupted.
#[derive(Debug, Clone, PartialEq)]
pub struct Resumable<T, C> {
    /// The data item.
    pub data: T,
    /// The context for resuming after this item.
    pub context: C,
}

impl<T, C> Resumable<T, C> {
    /// Creates a new resumable item.
    pub fn new(data: T, context: C) -> Self {
        Self { data, context }
    }
}

/// Trait for reading data from a source.
///
/// Implementations provide streaming access to data with optional pagination.
/// Each item is paired with a context that can be used to resume reading
/// from that point if the stream is interrupted.
#[async_trait::async_trait]
pub trait DataInput: Send + Sync {
    /// The item type produced by this provider.
    type Datatype;
    /// The context type for read operations.
    type Context;

    /// Reads items from the source.
    ///
    /// Returns an input stream of [`Resumable`] items.
    /// The context represents the state needed to resume reading after that item.
    async fn read(
        &self,
        ctx: &Self::Context,
    ) -> Result<InputStream<Resumable<Self::Datatype, Self::Context>>>;
}

/// Trait for writing data to a sink.
///
/// Implementations accept batches of items for writing.
#[async_trait::async_trait]
pub trait DataOutput: Send + Sync {
    /// The item type accepted by this provider.
    type Datatype;

    /// Writes a batch of items to the sink.
    async fn write(&self, items: Vec<Self::Datatype>) -> Result<()>;
}
