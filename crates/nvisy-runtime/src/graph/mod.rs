//! Workflow graph structures and node types.
//!
//! This module provides the graph representation for workflows:
//! - [`WorkflowGraph`]: The main graph structure containing nodes and edges
//! - [`WorkflowMetadata`]: Metadata about the workflow
//! - [`Edge`]: Connections between nodes
//! - [`EdgeData`]: Data stored on edges in the underlying petgraph
//! - [`NodeId`]: Unique identifier for nodes
//! - [`NodeData`]: Data associated with each node (Input, Transformer, Output)
//! - [`CacheSlot`]: Named cache slot for in-memory data passing
//! - [`SwitchNode`]: Conditional routing based on data properties

mod edge;
pub mod input;
mod node;
pub mod output;
pub mod route;
pub mod transform;
mod workflow;

pub use edge::{Edge, EdgeData};
pub use input::{InputNode, InputSource};
pub use node::{Node, NodeCommon, NodeData, NodeId};
pub use output::{OutputDestination, OutputNode};
pub use route::{CacheSlot, SwitchBranch, SwitchCondition, SwitchNode};
pub use transform::TransformerConfig;
pub use workflow::{WorkflowGraph, WorkflowMetadata};
