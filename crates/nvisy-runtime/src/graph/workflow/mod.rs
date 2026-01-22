//! Workflow graph types.
//!
//! This module provides:
//! - [`WorkflowGraph`]: Runtime graph representation using petgraph
//! - [`WorkflowDefinition`]: Serializable JSON-friendly definition
//! - [`WorkflowMetadata`]: Workflow metadata (name, description, version, etc.)
//! - [`Node`], [`NodeId`], [`NodeData`]: Node types and identifiers
//! - [`Edge`], [`EdgeData`]: Edge types

mod definition;
mod edge;
mod metadata;
mod node;

pub use definition::WorkflowDefinition;
pub use edge::{Edge, EdgeData};
pub use metadata::WorkflowMetadata;
pub use node::{Node, NodeCommon, NodeData, NodeId};
