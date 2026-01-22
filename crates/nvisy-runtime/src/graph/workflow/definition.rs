//! Serializable workflow definition.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::edge::Edge;
use super::metadata::WorkflowMetadata;
use super::node::{NodeData, NodeId};
use crate::error::{WorkflowError, WorkflowResult};
use crate::graph::WorkflowGraph;

/// Serializable workflow definition.
///
/// This is the JSON-friendly representation of a workflow graph.
/// Use [`WorkflowGraph::to_definition`] and [`WorkflowGraph::from_definition`]
/// to convert between the two representations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    /// Nodes in the workflow, keyed by their ID.
    pub nodes: HashMap<NodeId, NodeData>,
    /// Edges connecting nodes.
    pub edges: Vec<Edge>,
    /// Workflow metadata.
    #[serde(default)]
    pub metadata: WorkflowMetadata,
}

impl WorkflowDefinition {
    /// Creates a new empty workflow definition.
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
            metadata: WorkflowMetadata::default(),
        }
    }

    /// Creates a workflow definition with metadata.
    pub fn with_metadata(metadata: WorkflowMetadata) -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
            metadata,
        }
    }

    /// Converts this definition into a workflow graph.
    ///
    /// Returns an error if any edge references a non-existent node.
    pub fn into_graph(self) -> WorkflowResult<WorkflowGraph> {
        WorkflowGraph::from_definition(self)
    }
}

impl Default for WorkflowDefinition {
    fn default() -> Self {
        Self::new()
    }
}

impl TryFrom<WorkflowDefinition> for WorkflowGraph {
    type Error = WorkflowError;

    fn try_from(definition: WorkflowDefinition) -> Result<Self, Self::Error> {
        Self::from_definition(definition)
    }
}

impl From<&WorkflowGraph> for WorkflowDefinition {
    fn from(graph: &WorkflowGraph) -> Self {
        graph.to_definition()
    }
}

impl From<WorkflowGraph> for WorkflowDefinition {
    fn from(graph: WorkflowGraph) -> Self {
        graph.to_definition()
    }
}
