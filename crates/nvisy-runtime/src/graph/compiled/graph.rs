//! Compiled workflow graph.

use std::collections::HashMap;

use petgraph::Direction;
use petgraph::graph::{DiGraph, NodeIndex};

use super::input::CompiledInput;
use super::node::CompiledNode;
use super::output::CompiledOutput;
use super::route::CompiledSwitch;
use crate::graph::definition::{
    ContentTypeCategory, ContentTypeCondition, EdgeData, NodeId, SwitchCondition, WorkflowMetadata,
};

/// A compiled workflow graph ready for execution.
///
/// This is the runtime representation of a workflow after compilation.
/// All cache slots are resolved into direct edges, and all node definitions
/// are compiled into their executable forms.
pub struct CompiledGraph {
    /// The underlying directed graph.
    graph: DiGraph<CompiledNode, EdgeData>,
    /// Map from node IDs to graph indices.
    node_indices: HashMap<NodeId, NodeIndex>,
    /// Map from graph indices to node IDs.
    index_to_id: HashMap<NodeIndex, NodeId>,
    /// Workflow metadata.
    metadata: WorkflowMetadata,
}

impl CompiledGraph {
    /// Creates a new compiled graph.
    pub fn new(
        graph: DiGraph<CompiledNode, EdgeData>,
        node_indices: HashMap<NodeId, NodeIndex>,
        metadata: WorkflowMetadata,
    ) -> Self {
        let index_to_id = node_indices.iter().map(|(k, v)| (*v, *k)).collect();
        Self {
            graph,
            node_indices,
            index_to_id,
            metadata,
        }
    }

    /// Returns the workflow metadata.
    pub fn metadata(&self) -> &WorkflowMetadata {
        &self.metadata
    }

    /// Returns the number of nodes in the graph.
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Returns the number of edges in the graph.
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    /// Returns a reference to a node by ID.
    pub fn node(&self, id: &NodeId) -> Option<&CompiledNode> {
        self.node_indices
            .get(id)
            .and_then(|&idx| self.graph.node_weight(idx))
    }

    /// Returns a mutable reference to a node by ID.
    pub fn node_mut(&mut self, id: &NodeId) -> Option<&mut CompiledNode> {
        self.node_indices
            .get(id)
            .copied()
            .and_then(|idx| self.graph.node_weight_mut(idx))
    }

    /// Returns the node ID for a graph index.
    pub fn node_id(&self, index: NodeIndex) -> Option<NodeId> {
        self.index_to_id.get(&index).copied()
    }

    /// Returns the graph index for a node ID.
    pub fn node_index(&self, id: &NodeId) -> Option<NodeIndex> {
        self.node_indices.get(id).copied()
    }

    /// Returns an iterator over all node IDs.
    pub fn node_ids(&self) -> impl Iterator<Item = &NodeId> {
        self.node_indices.keys()
    }

    /// Returns an iterator over all nodes with their IDs.
    pub fn nodes(&self) -> impl Iterator<Item = (&NodeId, &CompiledNode)> {
        self.node_indices
            .iter()
            .filter_map(|(id, &idx)| self.graph.node_weight(idx).map(|node| (id, node)))
    }

    /// Returns an iterator over input nodes.
    pub fn input_nodes(&self) -> impl Iterator<Item = (&NodeId, &CompiledInput)> {
        self.nodes()
            .filter_map(|(id, node)| node.as_input().map(|input| (id, input)))
    }

    /// Returns an iterator over output nodes.
    pub fn output_nodes(&self) -> impl Iterator<Item = (&NodeId, &CompiledOutput)> {
        self.nodes()
            .filter_map(|(id, node)| node.as_output().map(|output| (id, output)))
    }

    /// Returns the predecessors (incoming nodes) of a node.
    pub fn predecessors(&self, id: &NodeId) -> impl Iterator<Item = &NodeId> {
        self.node_indices.get(id).into_iter().flat_map(|&idx| {
            self.graph
                .neighbors_directed(idx, Direction::Incoming)
                .filter_map(|pred_idx| self.index_to_id.get(&pred_idx))
        })
    }

    /// Returns the successors (outgoing nodes) of a node.
    pub fn successors(&self, id: &NodeId) -> impl Iterator<Item = &NodeId> {
        self.node_indices.get(id).into_iter().flat_map(|&idx| {
            self.graph
                .neighbors_directed(idx, Direction::Outgoing)
                .filter_map(|succ_idx| self.index_to_id.get(&succ_idx))
        })
    }

    /// Returns the edge data between two nodes, if an edge exists.
    pub fn edge(&self, from: &NodeId, to: &NodeId) -> Option<&EdgeData> {
        let from_idx = self.node_indices.get(from)?;
        let to_idx = self.node_indices.get(to)?;
        self.graph
            .find_edge(*from_idx, *to_idx)
            .and_then(|e| self.graph.edge_weight(e))
    }

    /// Returns topologically sorted node IDs (sources first).
    ///
    /// Returns `None` if the graph contains a cycle.
    pub fn topological_order(&self) -> Option<Vec<NodeId>> {
        petgraph::algo::toposort(&self.graph, None)
            .ok()
            .map(|indices| {
                indices
                    .into_iter()
                    .filter_map(|idx| self.index_to_id.get(&idx).copied())
                    .collect()
            })
    }

    /// Consumes the graph and returns ownership of all nodes.
    ///
    /// Returns a map from node IDs to compiled nodes.
    pub fn into_nodes(mut self) -> HashMap<NodeId, CompiledNode> {
        let mut nodes = HashMap::with_capacity(self.node_indices.len());
        for (id, &idx) in &self.node_indices {
            if let Some(node) = self.graph.node_weight_mut(idx) {
                // Use mem::replace with a placeholder to take ownership
                // This is safe because we won't access the graph again
                let placeholder = CompiledNode::Switch(CompiledSwitch::new(
                    SwitchCondition::ContentType(ContentTypeCondition {
                        category: ContentTypeCategory::Other,
                    }),
                    String::new(),
                    String::new(),
                ));
                let owned = std::mem::replace(node, placeholder);
                nodes.insert(*id, owned);
            }
        }
        nodes
    }

    /// Returns a reference to the underlying petgraph.
    pub fn inner(&self) -> &DiGraph<CompiledNode, EdgeData> {
        &self.graph
    }

    /// Returns a mutable reference to the underlying petgraph.
    pub fn inner_mut(&mut self) -> &mut DiGraph<CompiledNode, EdgeData> {
        &mut self.graph
    }
}

impl std::fmt::Debug for CompiledGraph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompiledGraph")
            .field("node_count", &self.graph.node_count())
            .field("edge_count", &self.graph.edge_count())
            .field("metadata", &self.metadata)
            .finish()
    }
}
