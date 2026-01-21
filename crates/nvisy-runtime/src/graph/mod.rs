//! Workflow graph structures and node types.
//!
//! This module provides the graph representation for workflows:
//! - [`WorkflowGraph`]: The main graph structure containing nodes and edges
//! - [`WorkflowMetadata`]: Metadata about the workflow
//! - [`Edge`]: Connections between nodes
//! - [`EdgeData`]: Data stored on edges in the underlying petgraph
//! - [`NodeId`]: Unique identifier for nodes
//! - [`NodeData`]: Data associated with each node (Input, Transformer, Output)

mod data;
mod edge;
mod id;
pub mod input;
pub mod output;
pub mod transformer;
mod workflow;

pub use data::NodeData;
pub use edge::Edge;
pub use id::NodeId;
pub use input::InputNode;
pub use output::OutputNode;
pub use transformer::{TransformerConfig, TransformerNode};
pub use workflow::{EdgeData, WorkflowGraph, WorkflowMetadata};
