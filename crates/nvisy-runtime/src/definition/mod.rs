//! Workflow definition types.
//!
//! This module contains serializable, frontend-friendly types for defining workflows.
//! These types are designed for:
//! - Easy serialization to/from JSON
//! - Frontend consumption and editing
//! - Storage in databases
//!
//! To execute a workflow, definitions must be compiled into runtime types
//! using the [`crate::engine::Engine`].

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

mod edge;
mod input;
mod metadata;
mod node;
mod output;
mod route;
mod transform;
mod util;

pub use edge::Edge;
pub use input::Input;
pub use metadata::WorkflowMetadata;
pub use node::{Node, NodeId, NodeKind};
pub use output::Output;
pub use route::{
    CacheSlot, FileCategory, FileCategoryCondition, LanguageCondition, SwitchCondition, SwitchDef,
};
pub use transform::{
    AnalyzeTask, Chunk, ChunkStrategy, ConvertTask, Derive, DeriveTask, Embedding, Enrich,
    EnrichTask, Extract, ExtractTask, ImageEnrichTask, Partition, PartitionStrategy,
    TableConvertTask, TableEnrichTask, TextConvertTask, Transformer,
};
pub use util::Position;

/// Serializable workflow definition.
///
/// This is the JSON-friendly representation of a workflow graph.
/// It contains all the information needed to compile and execute a workflow.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Workflow {
    /// Nodes in the workflow, keyed by their ID.
    pub nodes: HashMap<NodeId, Node>,
    /// Edges connecting nodes.
    pub edges: Vec<Edge>,
    /// Workflow metadata.
    #[serde(default)]
    pub metadata: WorkflowMetadata,
}

impl Workflow {
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
    pub fn add_node_def(&mut self, id: NodeId, def: NodeKind) -> &mut Self {
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
        self.nodes.iter().filter(|(_, node)| node.is_input())
    }

    /// Returns an iterator over output nodes.
    pub fn output_nodes(&self) -> impl Iterator<Item = (&NodeId, &Node)> {
        self.nodes.iter().filter(|(_, node)| node.is_output())
    }

    /// Returns an iterator over transform nodes.
    pub fn transform_nodes(&self) -> impl Iterator<Item = (&NodeId, &Node)> {
        self.nodes.iter().filter(|(_, node)| node.is_transform())
    }

    /// Returns an iterator over switch nodes.
    pub fn switch_nodes(&self) -> impl Iterator<Item = (&NodeId, &Node)> {
        self.nodes.iter().filter(|(_, node)| node.is_switch())
    }
}

impl Default for Workflow {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;

    /// Creates a deterministic NodeId for testing.
    fn test_node_id(n: u128) -> NodeId {
        NodeId::from_uuid(Uuid::from_u128(n))
    }

    fn input_node_cache(slot: &str) -> Node {
        Node::new(NodeKind::Input(Input::CacheSlot(CacheSlot {
            slot: slot.to_string(),
            priority: None,
        })))
    }

    fn output_node_cache(slot: &str) -> Node {
        Node::new(NodeKind::Output(Output::Cache(CacheSlot {
            slot: slot.to_string(),
            priority: None,
        })))
    }

    fn transform_node_partition() -> Node {
        Node::new(NodeKind::Transform(Transformer::Partition(Partition {
            strategy: Default::default(),
            include_page_breaks: false,
            discard_unsupported: false,
        })))
    }

    #[test]
    fn test_workflow_definition_new() {
        let def = Workflow::new();
        assert!(def.nodes.is_empty());
        assert!(def.edges.is_empty());
    }

    #[test]
    fn test_workflow_definition_add_node() {
        let mut def = Workflow::new();
        let id = test_node_id(1);
        def.add_node(id, input_node_cache("test"));
        assert_eq!(def.nodes.len(), 1);
        assert!(def.nodes.contains_key(&id));
    }

    #[test]
    fn test_workflow_definition_connect() {
        let mut def = Workflow::new();
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
    fn test_workflow_definition_node_iterators() {
        let mut def = Workflow::new();
        def.add_node(test_node_id(1), input_node_cache("in"))
            .add_node(test_node_id(2), transform_node_partition())
            .add_node(test_node_id(3), output_node_cache("out"));

        assert_eq!(def.input_nodes().count(), 1);
        assert_eq!(def.transform_nodes().count(), 1);
        assert_eq!(def.output_nodes().count(), 1);
    }

    #[test]
    fn test_workflow_definition_serialization() {
        let mut def = Workflow::new();
        let id1 = test_node_id(1);
        let id2 = test_node_id(2);
        def.add_node(id1, input_node_cache("in"))
            .add_node(id2, output_node_cache("out"))
            .connect(id1, id2);

        // Serialize to JSON
        let json = serde_json::to_string(&def).expect("serialization failed");

        // Deserialize back
        let deserialized: Workflow = serde_json::from_str(&json).expect("deserialization failed");

        assert_eq!(def.nodes.len(), deserialized.nodes.len());
        assert_eq!(def.edges.len(), deserialized.edges.len());
    }
}
