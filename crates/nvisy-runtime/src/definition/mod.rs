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
pub use input::{Input, InputParams, ProviderInput};
pub use metadata::WorkflowMetadata;
pub use node::{Node, NodeId, NodeKind};
pub use output::{Output, OutputParams, ProviderOutput};
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
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Workflow {
    /// Nodes in the workflow, keyed by their ID.
    pub nodes: HashMap<NodeId, Node>,
    /// Edges connecting nodes.
    pub edges: Vec<Edge>,
    /// Workflow metadata.
    #[serde(default)]
    pub metadata: WorkflowMetadata,
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;

    /// Creates a deterministic NodeId for testing.
    fn test_node_id(n: u128) -> NodeId {
        Uuid::from_u128(n).into()
    }

    fn input_node_cache(slot: &str) -> Node {
        Node {
            name: None,
            description: None,
            position: None,
            kind: NodeKind::Input(Input::CacheSlot(CacheSlot {
                slot: slot.to_string(),
                priority: None,
            })),
        }
    }

    fn output_node_cache(slot: &str) -> Node {
        Node {
            name: None,
            description: None,
            position: None,
            kind: NodeKind::Output(Output::CacheSlot(CacheSlot {
                slot: slot.to_string(),
                priority: None,
            })),
        }
    }

    fn transform_node_partition() -> Node {
        Node {
            name: None,
            description: None,
            position: None,
            kind: NodeKind::Transform(Transformer::Partition(Partition {
                strategy: Default::default(),
                include_page_breaks: false,
                discard_unsupported: false,
            })),
        }
    }

    #[test]
    fn test_workflow_definition_new() {
        let def = Workflow::default();
        assert!(def.nodes.is_empty());
        assert!(def.edges.is_empty());
    }

    #[test]
    fn test_workflow_definition_add_node() {
        let mut def = Workflow::default();
        let id = test_node_id(1);
        def.nodes.insert(id, input_node_cache("test"));
        assert_eq!(def.nodes.len(), 1);
        assert!(def.nodes.contains_key(&id));
    }

    #[test]
    fn test_workflow_definition_connect() {
        let mut def = Workflow::default();
        let id1 = test_node_id(1);
        let id2 = test_node_id(2);
        def.nodes.insert(id1, input_node_cache("in"));
        def.nodes.insert(id2, output_node_cache("out"));
        def.edges.push(Edge {
            from: id1,
            to: id2,
            from_port: None,
            to_port: None,
        });

        assert_eq!(def.edges.len(), 1);
        assert_eq!(def.edges[0].from, id1);
        assert_eq!(def.edges[0].to, id2);
    }

    #[test]
    fn test_workflow_definition_node_iterators() {
        let mut def = Workflow::default();
        def.nodes.insert(test_node_id(1), input_node_cache("in"));
        def.nodes
            .insert(test_node_id(2), transform_node_partition());
        def.nodes.insert(test_node_id(3), output_node_cache("out"));

        assert_eq!(def.nodes.values().filter(|n| n.is_input()).count(), 1);
        assert_eq!(def.nodes.values().filter(|n| n.is_transform()).count(), 1);
        assert_eq!(def.nodes.values().filter(|n| n.is_output()).count(), 1);
    }

    #[test]
    fn test_workflow_definition_serialization() {
        let mut def = Workflow::default();
        let id1 = test_node_id(1);
        let id2 = test_node_id(2);
        def.nodes.insert(id1, input_node_cache("in"));
        def.nodes.insert(id2, output_node_cache("out"));
        def.edges.push(Edge {
            from: id1,
            to: id2,
            from_port: None,
            to_port: None,
        });

        // Serialize to JSON
        let json = serde_json::to_string(&def).expect("serialization failed");

        // Deserialize back
        let deserialized: Workflow = serde_json::from_str(&json).expect("deserialization failed");

        assert_eq!(def.nodes.len(), deserialized.nodes.len());
        assert_eq!(def.edges.len(), deserialized.edges.len());
    }
}
