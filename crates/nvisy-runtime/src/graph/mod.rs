//! Workflow graph structures and node types.
//!
//! This module provides the graph representation for workflows:
//! - [`WorkflowGraph`]: The main graph structure containing nodes and edges
//! - [`WorkflowDefinition`]: Serializable workflow definition (JSON-friendly)
//! - [`WorkflowMetadata`]: Metadata about the workflow
//! - [`Edge`]: Connections between nodes
//! - [`EdgeData`]: Data stored on edges in the underlying petgraph
//! - [`NodeId`]: Unique identifier for nodes
//! - [`NodeData`]: Data associated with each node (Input, Transformer, Output)
//! - [`CacheSlot`]: Named cache slot for in-memory data passing
//! - [`SwitchNode`]: Conditional routing based on data properties

mod graph;
pub mod input;
pub mod output;
pub mod route;
pub mod transform;
pub mod workflow;

pub use graph::WorkflowGraph;
pub use input::{InputNode, InputSource};
pub use output::{OutputDestination, OutputNode};
pub use route::{CacheSlot, SwitchBranch, SwitchCondition, SwitchNode};
pub use transform::TransformerConfig;
pub use workflow::{Edge, EdgeData, Node, NodeCommon, NodeData, NodeId};
pub use workflow::{WorkflowDefinition, WorkflowMetadata};
