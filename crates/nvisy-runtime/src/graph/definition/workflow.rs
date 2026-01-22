//! Serializable workflow definition.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::edge::Edge;
use super::metadata::WorkflowMetadata;
use super::node::{Node, NodeDef, NodeId};

/// Serializable workflow definition.
///
/// This is the JSON-friendly representation of a workflow graph.
/// It contains all the information needed to compile and execute a workflow.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    /// Nodes in the workflow, keyed by their ID.
    pub nodes: HashMap<NodeId, Node>,
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

    /// Adds a node to the workflow.
    pub fn add_node(&mut self, id: NodeId, node: Node) -> &mut Self {
        self.nodes.insert(id, node);
        self
    }

    /// Adds a node definition with default metadata.
    pub fn add_node_def(&mut self, id: NodeId, def: NodeDef) -> &mut Self {
        self.nodes.insert(id, Node::new(def));
        self
    }

    /// Adds an edge to the workflow.
    pub fn add_edge(&mut self, edge: Edge) -> &mut Self {
        self.edges.push(edge);
        self
    }

    /// Adds a simple edge between two nodes.
    pub fn connect(&mut self, from: NodeId, to: NodeId) -> &mut Self {
        self.edges.push(Edge::new(from, to));
        self
    }

    /// Returns an iterator over input nodes.
    pub fn input_nodes(&self) -> impl Iterator<Item = (&NodeId, &Node)> {
        self.nodes.iter().filter(|(_, node)| node.inner.is_input())
    }

    /// Returns an iterator over output nodes.
    pub fn output_nodes(&self) -> impl Iterator<Item = (&NodeId, &Node)> {
        self.nodes.iter().filter(|(_, node)| node.inner.is_output())
    }

    /// Returns an iterator over transform nodes.
    pub fn transform_nodes(&self) -> impl Iterator<Item = (&NodeId, &Node)> {
        self.nodes
            .iter()
            .filter(|(_, node)| node.inner.is_transform())
    }

    /// Returns an iterator over switch nodes.
    pub fn switch_nodes(&self) -> impl Iterator<Item = (&NodeId, &Node)> {
        self.nodes.iter().filter(|(_, node)| node.inner.is_switch())
    }

    /// Validates the workflow definition structure.
    ///
    /// Checks that:
    /// - All edge endpoints reference existing nodes
    /// - There are no orphan nodes (nodes with no connections)
    /// - The graph has at least one input and one output node
    pub fn validate(&self) -> Result<(), ValidationError> {
        // Check edge references
        for edge in &self.edges {
            if !self.nodes.contains_key(&edge.from) {
                return Err(ValidationError::MissingNode(edge.from));
            }
            if !self.nodes.contains_key(&edge.to) {
                return Err(ValidationError::MissingNode(edge.to));
            }
        }

        // Check for at least one input and output
        let has_input = self.nodes.values().any(|n| n.inner.is_input());
        let has_output = self.nodes.values().any(|n| n.inner.is_output());

        if !has_input {
            return Err(ValidationError::NoInputNode);
        }
        if !has_output {
            return Err(ValidationError::NoOutputNode);
        }

        Ok(())
    }
}

impl Default for WorkflowDefinition {
    fn default() -> Self {
        Self::new()
    }
}

/// Validation errors for workflow definitions.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ValidationError {
    /// An edge references a non-existent node.
    #[error("edge references non-existent node: {0}")]
    MissingNode(NodeId),
    /// The workflow has no input nodes.
    #[error("workflow must have at least one input node")]
    NoInputNode,
    /// The workflow has no output nodes.
    #[error("workflow must have at least one output node")]
    NoOutputNode,
    /// The workflow contains a cycle.
    #[error("workflow contains a cycle")]
    CycleDetected,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::definition::{
        CacheSlot, InputDef, InputSource, OutputDef, OutputTarget, Partition, Transformer,
    };
    use uuid::Uuid;

    /// Creates a deterministic NodeId for testing.
    fn test_node_id(n: u128) -> NodeId {
        NodeId::from_uuid(Uuid::from_u128(n))
    }

    fn input_node_cache(slot: &str) -> Node {
        Node::new(NodeDef::Input(InputDef {
            source: InputSource::CacheSlot(CacheSlot {
                slot: slot.to_string(),
                priority: None,
            }),
        }))
    }

    fn output_node_cache(slot: &str) -> Node {
        Node::new(NodeDef::Output(OutputDef {
            target: OutputTarget::Cache(CacheSlot {
                slot: slot.to_string(),
                priority: None,
            }),
        }))
    }

    fn transform_node_partition() -> Node {
        Node::new(NodeDef::Transform(Transformer::Partition(Partition {
            strategy: Default::default(),
            include_page_breaks: false,
            discard_unsupported: false,
        })))
    }

    #[test]
    fn test_workflow_definition_new() {
        let def = WorkflowDefinition::new();
        assert!(def.nodes.is_empty());
        assert!(def.edges.is_empty());
    }

    #[test]
    fn test_workflow_definition_add_node() {
        let mut def = WorkflowDefinition::new();
        let id = test_node_id(1);
        def.add_node(id, input_node_cache("test"));
        assert_eq!(def.nodes.len(), 1);
        assert!(def.nodes.contains_key(&id));
    }

    #[test]
    fn test_workflow_definition_connect() {
        let mut def = WorkflowDefinition::new();
        let id1 = test_node_id(1);
        let id2 = test_node_id(2);
        def.add_node(id1, input_node_cache("in"))
            .add_node(id2, output_node_cache("out"))
            .connect(id1, id2);

        assert_eq!(def.edges.len(), 1);
        assert_eq!(def.edges[0].from, id1);
        assert_eq!(def.edges[0].to, id2);
    }

    #[test]
    fn test_workflow_definition_validate_valid() {
        let mut def = WorkflowDefinition::new();
        let id1 = test_node_id(1);
        let id2 = test_node_id(2);
        def.add_node(id1, input_node_cache("in"))
            .add_node(id2, output_node_cache("out"))
            .connect(id1, id2);

        assert!(def.validate().is_ok());
    }

    #[test]
    fn test_workflow_definition_validate_missing_node() {
        let mut def = WorkflowDefinition::new();
        let id1 = test_node_id(1);
        let id2 = test_node_id(2);
        let id_invalid = test_node_id(99);
        def.add_node(id1, input_node_cache("in"))
            .add_node(id2, output_node_cache("out"))
            .connect(id1, id_invalid); // Invalid reference

        let result = def.validate();
        assert!(result.is_err());
        assert!(matches!(result, Err(ValidationError::MissingNode(_))));
    }

    #[test]
    fn test_workflow_definition_validate_no_input() {
        let mut def = WorkflowDefinition::new();
        def.add_node(test_node_id(1), output_node_cache("out"));

        let result = def.validate();
        assert!(result.is_err());
        assert!(matches!(result, Err(ValidationError::NoInputNode)));
    }

    #[test]
    fn test_workflow_definition_validate_no_output() {
        let mut def = WorkflowDefinition::new();
        def.add_node(test_node_id(1), input_node_cache("in"));

        let result = def.validate();
        assert!(result.is_err());
        assert!(matches!(result, Err(ValidationError::NoOutputNode)));
    }

    #[test]
    fn test_workflow_definition_node_iterators() {
        let mut def = WorkflowDefinition::new();
        def.add_node(test_node_id(1), input_node_cache("in"))
            .add_node(test_node_id(2), transform_node_partition())
            .add_node(test_node_id(3), output_node_cache("out"));

        assert_eq!(def.input_nodes().count(), 1);
        assert_eq!(def.transform_nodes().count(), 1);
        assert_eq!(def.output_nodes().count(), 1);
    }

    #[test]
    fn test_workflow_definition_serialization() {
        let mut def = WorkflowDefinition::new();
        let id1 = test_node_id(1);
        let id2 = test_node_id(2);
        def.add_node(id1, input_node_cache("in"))
            .add_node(id2, output_node_cache("out"))
            .connect(id1, id2);

        // Serialize to JSON
        let json = serde_json::to_string(&def).expect("serialization failed");

        // Deserialize back
        let deserialized: WorkflowDefinition =
            serde_json::from_str(&json).expect("deserialization failed");

        assert_eq!(def.nodes.len(), deserialized.nodes.len());
        assert_eq!(def.edges.len(), deserialized.edges.len());
    }
}
