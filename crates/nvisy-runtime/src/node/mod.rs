//! Node types for workflow graphs.
//!
//! This module provides the core node abstractions:
//! - [`NodeId`]: Unique identifier for nodes
//! - [`NodeData`]: Data associated with each node (Source, Transformer, Sink)

mod data;
mod id;

pub use data::{
    NodeData, SinkKind, SinkNode, SourceKind, SourceNode, TransformerKind, TransformerNode,
};
pub use id::NodeId;
