//! Workflow graph definition.

use std::collections::HashMap;

use petgraph::Direction;
use petgraph::algo::{is_cyclic_directed, toposort};
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use semver::Version;
use serde::{Deserialize, Serialize};

use super::{Edge, NodeData, NodeId};
use crate::error::{WorkflowError, WorkflowResult};

/// Workflow metadata.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct WorkflowMetadata {
    /// Workflow name (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Workflow description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Workflow version (semver, optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<Version>,
    /// Tags for organization.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Author identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Creation timestamp (ISO 8601).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    /// Last update timestamp (ISO 8601).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

impl WorkflowMetadata {
    /// Creates a new empty metadata.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the workflow name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Sets the workflow description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Sets the workflow version.
    pub fn with_version(mut self, version: Version) -> Self {
        self.version = Some(version);
        self
    }

    /// Sets the author.
    pub fn with_author(mut self, author: impl Into<String>) -> Self {
        self.author = Some(author.into());
        self
    }

    /// Adds tags.
    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags = tags.into_iter().map(Into::into).collect();
        self
    }
}

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

/// Edge data stored in the graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct EdgeData {
    /// Optional port/slot name on the source node.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_port: Option<String>,
    /// Optional port/slot name on the target node.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_port: Option<String>,
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

    /// Validates the workflow graph structure and constraints.
    pub fn validate(&self) -> WorkflowResult<()> {
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
}

impl Serialize for WorkflowGraph {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("WorkflowGraph", 3)?;

        // Serialize nodes as a map of NodeId -> NodeData
        let nodes: HashMap<NodeId, &NodeData> = self.nodes().collect();
        state.serialize_field("nodes", &nodes)?;

        // Serialize edges
        let edges: Vec<Edge> = self.edges().collect();
        state.serialize_field("edges", &edges)?;

        state.serialize_field("metadata", &self.metadata)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for WorkflowGraph {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct WorkflowGraphData {
            nodes: HashMap<NodeId, NodeData>,
            edges: Vec<Edge>,
            #[serde(default)]
            metadata: WorkflowMetadata,
        }

        let data = WorkflowGraphData::deserialize(deserializer)?;
        let mut graph = WorkflowGraph::with_metadata(data.metadata);

        for (id, node_data) in data.nodes {
            graph.add_node_with_id(id, node_data);
        }

        for edge in data.edges {
            graph.add_edge(edge).map_err(serde::de::Error::custom)?;
        }

        Ok(graph)
    }
}
