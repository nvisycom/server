//! Workflow graph structures.
//!
//! This module provides the graph representation for workflows:
//! - [`WorkflowGraph`]: The main graph structure containing nodes and edges
//! - [`WorkflowMetadata`]: Metadata about the workflow
//! - [`Edge`]: Connections between nodes
//! - [`EdgeData`]: Data stored on edges in the underlying petgraph

mod edge;
mod workflow;

pub use edge::Edge;
pub use workflow::{EdgeData, WorkflowGraph, WorkflowMetadata};
