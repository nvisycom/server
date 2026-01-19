//! Workflow graph structures.
//!
//! This module provides the graph representation for workflows:
//! - [`WorkflowGraph`]: The main graph structure containing nodes and edges
//! - [`Edge`]: Connections between nodes

mod edge;
mod workflow;

pub use edge::Edge;
pub use workflow::WorkflowGraph;
