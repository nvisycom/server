//! Workflow graph runtime representation.

use std::collections::{HashMap, HashSet};

use petgraph::Direction;
use petgraph::algo::{is_cyclic_directed, toposort};
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use uuid::Uuid;

use super::input::InputSource;
use super::output::OutputDestination;
use super::transform::TransformerConfig;
use super::workflow::{Edge, EdgeData, NodeData, NodeId, WorkflowDefinition, WorkflowMetadata};
use crate::error::{WorkflowError, WorkflowResult};
use crate::provider::CredentialsRegistry;

/// A workflow graph containing nodes and edges.
///
/// Internally uses petgraph's `DiGraph` for efficient graph operations.
#[derive(Debug, Clone, Default)]
pub struct WorkflowGraph {
    /// The underlying directed graph.
    graph: DiGraph<NodeData, EdgeData>,
    /// Mapping from NodeId to petgraph's NodeIndex.
    node_indices: HashMap<NodeId, NodeIndex>,
    /// Reverse mapping from NodeIndex to NodeId.
    index_to_id: HashMap<NodeIndex, NodeId>,
    /// Workflow metadata.
    pub metadata: WorkflowMetadata,
}

impl WorkflowGraph {
    /// Creates a new empty workflow graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new workflow graph with metadata.
    pub fn with_metadata(metadata: WorkflowMetadata) -> Self {
        Self {
            metadata,
            ..Default::default()
        }
    }

    /// Returns the number of nodes in the graph.
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Returns the number of edges in the graph.
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    /// Returns whether the graph is empty.
    pub fn is_empty(&self) -> bool {
        self.graph.node_count() == 0
    }

    /// Adds a node to the graph and returns its ID.
    pub fn add_node(&mut self, data: impl Into<NodeData>) -> NodeId {
        let id = NodeId::new();
        let index = self.graph.add_node(data.into());
        self.node_indices.insert(id, index);
        self.index_to_id.insert(index, id);
        id
    }

    /// Adds a node with a specific ID.
    pub fn add_node_with_id(&mut self, id: NodeId, data: impl Into<NodeData>) {
        let index = self.graph.add_node(data.into());
        self.node_indices.insert(id, index);
        self.index_to_id.insert(index, id);
    }

    /// Removes a node and all its connected edges.
    pub fn remove_node(&mut self, id: NodeId) -> Option<NodeData> {
        let index = self.node_indices.remove(&id)?;
        self.index_to_id.remove(&index);
        self.graph.remove_node(index)
    }

    /// Returns a reference to a node's data.
    pub fn get_node(&self, id: NodeId) -> Option<&NodeData> {
        let index = self.node_indices.get(&id)?;
        self.graph.node_weight(*index)
    }

    /// Returns a mutable reference to a node's data.
    pub fn get_node_mut(&mut self, id: NodeId) -> Option<&mut NodeData> {
        let index = self.node_indices.get(&id)?;
        self.graph.node_weight_mut(*index)
    }

    /// Returns whether a node exists.
    pub fn contains_node(&self, id: NodeId) -> bool {
        self.node_indices.contains_key(&id)
    }

    /// Returns an iterator over all nodes.
    pub fn nodes(&self) -> impl Iterator<Item = (NodeId, &NodeData)> {
        self.graph.node_indices().filter_map(|index| {
            let id = self.index_to_id.get(&index)?;
            let data = self.graph.node_weight(index)?;
            Some((*id, data))
        })
    }

    /// Returns an iterator over all node IDs.
    pub fn node_ids(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.node_indices.keys().copied()
    }

    /// Adds an edge between two nodes.
    pub fn add_edge(&mut self, edge: Edge) -> WorkflowResult<()> {
        let from_index = self.node_indices.get(&edge.from).ok_or_else(|| {
            WorkflowError::InvalidDefinition(format!("source node {} does not exist", edge.from))
        })?;
        let to_index = self.node_indices.get(&edge.to).ok_or_else(|| {
            WorkflowError::InvalidDefinition(format!("target node {} does not exist", edge.to))
        })?;

        let edge_data = EdgeData {
            from_port: edge.from_port,
            to_port: edge.to_port,
        };

        self.graph.add_edge(*from_index, *to_index, edge_data);
        Ok(())
    }

    /// Connects two nodes with a simple edge.
    pub fn connect(&mut self, from: NodeId, to: NodeId) -> WorkflowResult<()> {
        self.add_edge(Edge::new(from, to))
    }

    /// Returns an iterator over all edges.
    pub fn edges(&self) -> impl Iterator<Item = Edge> + '_ {
        self.graph.edge_references().filter_map(|edge_ref| {
            let from = *self.index_to_id.get(&edge_ref.source())?;
            let to = *self.index_to_id.get(&edge_ref.target())?;
            let data = edge_ref.weight();
            Some(Edge {
                from,
                to,
                from_port: data.from_port.clone(),
                to_port: data.to_port.clone(),
            })
        })
    }

    /// Returns edges originating from a node.
    pub fn outgoing_edges(&self, id: NodeId) -> impl Iterator<Item = Edge> + '_ {
        let index = self.node_indices.get(&id).copied();
        self.graph
            .edges_directed(
                index.unwrap_or(NodeIndex::new(usize::MAX)),
                Direction::Outgoing,
            )
            .filter_map(move |edge_ref| {
                let from = *self.index_to_id.get(&edge_ref.source())?;
                let to = *self.index_to_id.get(&edge_ref.target())?;
                let data = edge_ref.weight();
                Some(Edge {
                    from,
                    to,
                    from_port: data.from_port.clone(),
                    to_port: data.to_port.clone(),
                })
            })
    }

    /// Returns edges targeting a node.
    pub fn incoming_edges(&self, id: NodeId) -> impl Iterator<Item = Edge> + '_ {
        let index = self.node_indices.get(&id).copied();
        self.graph
            .edges_directed(
                index.unwrap_or(NodeIndex::new(usize::MAX)),
                Direction::Incoming,
            )
            .filter_map(move |edge_ref| {
                let from = *self.index_to_id.get(&edge_ref.source())?;
                let to = *self.index_to_id.get(&edge_ref.target())?;
                let data = edge_ref.weight();
                Some(Edge {
                    from,
                    to,
                    from_port: data.from_port.clone(),
                    to_port: data.to_port.clone(),
                })
            })
    }

    /// Returns all input nodes (nodes marked as Input or with no incoming edges).
    pub fn input_nodes(&self) -> Vec<NodeId> {
        self.graph
            .node_indices()
            .filter_map(|index| {
                let id = self.index_to_id.get(&index)?;
                let data = self.graph.node_weight(index)?;
                if data.is_input()
                    || self
                        .graph
                        .edges_directed(index, Direction::Incoming)
                        .next()
                        .is_none()
                {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Returns all output nodes (nodes marked as Output or with no outgoing edges).
    pub fn output_nodes(&self) -> Vec<NodeId> {
        self.graph
            .node_indices()
            .filter_map(|index| {
                let id = self.index_to_id.get(&index)?;
                let data = self.graph.node_weight(index)?;
                if data.is_output()
                    || self
                        .graph
                        .edges_directed(index, Direction::Outgoing)
                        .next()
                        .is_none()
                {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Collects all credentials IDs referenced by nodes in the workflow.
    ///
    /// Returns a set of unique credential UUIDs from input providers,
    /// output providers, and AI-powered transformers.
    pub fn credentials_ids(&self) -> HashSet<Uuid> {
        let mut ids = HashSet::new();

        for data in self.graph.node_weights() {
            match data {
                NodeData::Input(input) => {
                    if let InputSource::Provider(params) = &input.source {
                        ids.insert(params.credentials_id());
                    }
                }
                NodeData::Output(output) => {
                    if let OutputDestination::Provider(params) = &output.destination {
                        ids.insert(params.credentials_id());
                    }
                }
                NodeData::Transformer(config) => match config {
                    TransformerConfig::Embedding(c) => {
                        ids.insert(c.provider.credentials_id());
                    }
                    TransformerConfig::Enrich(c) => {
                        ids.insert(c.provider.credentials_id());
                    }
                    TransformerConfig::Extract(c) => {
                        ids.insert(c.provider.credentials_id());
                    }
                    TransformerConfig::Derive(c) => {
                        ids.insert(c.provider.credentials_id());
                    }
                    // Partition and Chunk don't require credentials
                    TransformerConfig::Partition(_) | TransformerConfig::Chunk(_) => {}
                },
            }
        }

        ids
    }

    /// Validates the workflow graph structure, constraints, and credentials.
    ///
    /// Checks that:
    /// - The graph has at least one node
    /// - There is at least one input and one output node
    /// - The graph is acyclic
    /// - Edge constraints are satisfied for each node type
    /// - All referenced credentials exist in the registry
    pub fn validate(&self, registry: &CredentialsRegistry) -> WorkflowResult<()> {
        // Must have at least one node
        if self.graph.node_count() == 0 {
            return Err(WorkflowError::InvalidDefinition(
                "workflow must have at least one node".into(),
            ));
        }

        // Must have at least one input node
        let has_input = self.graph.node_weights().any(|data| data.is_input());
        if !has_input {
            return Err(WorkflowError::InvalidDefinition(
                "workflow must have at least one input node".into(),
            ));
        }

        // Must have at least one output node
        let has_output = self.graph.node_weights().any(|data| data.is_output());
        if !has_output {
            return Err(WorkflowError::InvalidDefinition(
                "workflow must have at least one output node".into(),
            ));
        }

        // Check for cycles
        if is_cyclic_directed(&self.graph) {
            return Err(WorkflowError::InvalidDefinition(
                "cycle detected in workflow graph".into(),
            ));
        }

        // Validate edge constraints for each node
        for index in self.graph.node_indices() {
            let node_id = self
                .index_to_id
                .get(&index)
                .copied()
                .ok_or_else(|| WorkflowError::InvalidDefinition("invalid node index".into()))?;

            let data = self
                .graph
                .node_weight(index)
                .ok_or_else(|| WorkflowError::InvalidDefinition("missing node data".into()))?;

            let incoming_count = self
                .graph
                .edges_directed(index, Direction::Incoming)
                .count();
            let outgoing_count = self
                .graph
                .edges_directed(index, Direction::Outgoing)
                .count();

            // Input nodes must not have incoming edges
            if data.is_input() && incoming_count > 0 {
                return Err(WorkflowError::InvalidDefinition(format!(
                    "input node {} must not have incoming edges",
                    node_id
                )));
            }

            // Output nodes must not have outgoing edges
            if data.is_output() && outgoing_count > 0 {
                return Err(WorkflowError::InvalidDefinition(format!(
                    "output node {} must not have outgoing edges",
                    node_id
                )));
            }

            // Transformer nodes must have at least one incoming edge
            if data.is_transformer() && incoming_count == 0 {
                return Err(WorkflowError::InvalidDefinition(format!(
                    "transformer node {} must have at least one incoming edge",
                    node_id
                )));
            }

            // Transformer nodes must have at least one outgoing edge
            if data.is_transformer() && outgoing_count == 0 {
                return Err(WorkflowError::InvalidDefinition(format!(
                    "transformer node {} must have at least one outgoing edge",
                    node_id
                )));
            }
        }

        // Validate that all referenced credentials exist in the registry
        for credentials_id in self.credentials_ids() {
            registry.get(credentials_id)?;
        }

        Ok(())
    }

    /// Returns nodes in topological order.
    pub fn topological_order(&self) -> WorkflowResult<Vec<NodeId>> {
        toposort(&self.graph, None)
            .map(|indices| {
                indices
                    .into_iter()
                    .filter_map(|index| self.index_to_id.get(&index).copied())
                    .collect()
            })
            .map_err(|_| {
                WorkflowError::InvalidDefinition("cycle detected in workflow graph".into())
            })
    }

    /// Returns a reference to the underlying petgraph.
    pub fn inner(&self) -> &DiGraph<NodeData, EdgeData> {
        &self.graph
    }

    /// Returns a mutable reference to the underlying petgraph.
    pub fn inner_mut(&mut self) -> &mut DiGraph<NodeData, EdgeData> {
        &mut self.graph
    }

    /// Converts the workflow graph to a serializable definition.
    pub fn to_definition(&self) -> WorkflowDefinition {
        WorkflowDefinition {
            nodes: self.nodes().map(|(id, data)| (id, data.clone())).collect(),
            edges: self.edges().collect(),
            metadata: self.metadata.clone(),
        }
    }

    /// Creates a workflow graph from a definition.
    ///
    /// Returns an error if any edge references a non-existent node.
    pub fn from_definition(definition: WorkflowDefinition) -> WorkflowResult<Self> {
        let mut graph = Self::with_metadata(definition.metadata);

        for (id, node_data) in definition.nodes {
            graph.add_node_with_id(id, node_data);
        }

        for edge in definition.edges {
            graph.add_edge(edge)?;
        }

        Ok(graph)
    }
}
