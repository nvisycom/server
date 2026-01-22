//! Compiled workflow types for execution.
//!
//! This module contains runtime-optimized types for executing workflows.
//! These types are created by compiling workflow definitions and are
//! optimized for:
//! - Fast execution without lookups
//! - Pre-resolved cache slots
//! - Pre-instantiated providers and agents
//!
//! To create compiled types, use the [`crate::graph::compiler`] module.

mod graph;
mod input;
mod node;
mod output;
mod route;
mod transform;

pub use graph::CompiledGraph;
pub use input::{CompiledInput, DataStream, InputStream};
pub use node::CompiledNode;
pub use output::{CompiledOutput, DataSink, OutputStream};
pub use route::CompiledSwitch;
pub use transform::{
    ChunkProcessor, CompiledTransform, DeriveProcessor, EmbeddingProcessor, EnrichProcessor,
    ExtractProcessor, PartitionProcessor, Process,
};
