//! Workflow graph definition.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::{WorkflowError, WorkflowResult};
use crate::node::{NodeData, NodeId};

use super::Edge;

/// A workflow graph containing nodes and edges.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkflowGraph {
    /// Map of node IDs to their data.
    nodes: HashMap<NodeId, NodeData>,
    /// Edges connecting nodes.
    edges: Vec<Edge>,
    /// Workflow metadata.
    #[serde(default)]
    pub metadata: serde_json::Value,
}

impl WorkflowGraph {
    /// Creates a new empty workflow graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the number of nodes in the graph.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Returns the number of edges in the graph.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Returns whether the graph is empty.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Adds a node to the graph and returns its ID.
    pub fn add_node(&mut self, data: impl Into<NodeData>) -> NodeId {
        let id = NodeId::new();
        self.nodes.insert(id, data.into());
        id
    }

    /// Adds a node with a specific ID.
    pub fn add_node_with_id(&mut self, id: NodeId, data: impl Into<NodeData>) {
        self.nodes.insert(id, data.into());
    }

    /// Removes a node and all its connected edges.
    pub fn remove_node(&mut self, id: NodeId) -> Option<NodeData> {
        // Remove all edges connected to this node
        self.edges.retain(|e| e.from != id && e.to != id);
        self.nodes.remove(&id)
    }

    /// Returns a reference to a node's data.
    pub fn get_node(&self, id: NodeId) -> Option<&NodeData> {
        self.nodes.get(&id)
    }

    /// Returns a mutable reference to a node's data.
    pub fn get_node_mut(&mut self, id: NodeId) -> Option<&mut NodeData> {
        self.nodes.get_mut(&id)
    }

    /// Returns whether a node exists.
    pub fn contains_node(&self, id: NodeId) -> bool {
        self.nodes.contains_key(&id)
    }

    /// Returns an iterator over all nodes.
    pub fn nodes(&self) -> impl Iterator<Item = (NodeId, &NodeData)> {
        self.nodes.iter().map(|(&id, data)| (id, data))
    }

    /// Returns an iterator over all node IDs.
    pub fn node_ids(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.nodes.keys().copied()
    }

    /// Adds an edge between two nodes.
    pub fn add_edge(&mut self, edge: Edge) -> WorkflowResult<()> {
        // Validate that both nodes exist
        if !self.nodes.contains_key(&edge.from) {
            return Err(WorkflowError::InvalidDefinition(format!(
                "source node {} does not exist",
                edge.from
            )));
        }
        if !self.nodes.contains_key(&edge.to) {
            return Err(WorkflowError::InvalidDefinition(format!(
                "target node {} does not exist",
                edge.to
            )));
        }

        self.edges.push(edge);
        Ok(())
    }

    /// Connects two nodes with a simple edge.
    pub fn connect(&mut self, from: NodeId, to: NodeId) -> WorkflowResult<()> {
        self.add_edge(Edge::new(from, to))
    }

    /// Returns an iterator over all edges.
    pub fn edges(&self) -> impl Iterator<Item = &Edge> {
        self.edges.iter()
    }

    /// Returns edges originating from a node.
    pub fn outgoing_edges(&self, id: NodeId) -> impl Iterator<Item = &Edge> {
        self.edges.iter().filter(move |e| e.from == id)
    }

    /// Returns edges targeting a node.
    pub fn incoming_edges(&self, id: NodeId) -> impl Iterator<Item = &Edge> {
        self.edges.iter().filter(move |e| e.to == id)
    }

    /// Returns all source nodes (nodes with no incoming edges).
    pub fn source_nodes(&self) -> Vec<NodeId> {
        self.nodes
            .keys()
            .copied()
            .filter(|&id| {
                self.nodes.get(&id).is_some_and(|data| data.is_source())
                    || !self.edges.iter().any(|e| e.to == id)
            })
            .collect()
    }

    /// Returns all sink nodes (nodes with no outgoing edges).
    pub fn sink_nodes(&self) -> Vec<NodeId> {
        self.nodes
            .keys()
            .copied()
            .filter(|&id| {
                self.nodes.get(&id).is_some_and(|data| data.is_sink())
                    || !self.edges.iter().any(|e| e.from == id)
            })
            .collect()
    }

    /// Validates the workflow graph.
    pub fn validate(&self) -> WorkflowResult<()> {
        // Must have at least one node
        if self.nodes.is_empty() {
            return Err(WorkflowError::InvalidDefinition(
                "workflow must have at least one node".into(),
            ));
        }

        // Must have at least one source
        let sources: Vec<_> = self
            .nodes
            .iter()
            .filter(|(_, data)| data.is_source())
            .collect();
        if sources.is_empty() {
            return Err(WorkflowError::InvalidDefinition(
                "workflow must have at least one source node".into(),
            ));
        }

        // Must have at least one sink
        let sinks: Vec<_> = self
            .nodes
            .iter()
            .filter(|(_, data)| data.is_sink())
            .collect();
        if sinks.is_empty() {
            return Err(WorkflowError::InvalidDefinition(
                "workflow must have at least one sink node".into(),
            ));
        }

        // Check for cycles (simple DFS-based detection)
        self.check_cycles()?;

        Ok(())
    }

    /// Checks for cycles in the graph using DFS.
    fn check_cycles(&self) -> WorkflowResult<()> {
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum State {
            Unvisited,
            Visiting,
            Visited,
        }

        let mut states: HashMap<NodeId, State> = self
            .nodes
            .keys()
            .map(|&id| (id, State::Unvisited))
            .collect();

        fn dfs(
            graph: &WorkflowGraph,
            node: NodeId,
            states: &mut HashMap<NodeId, State>,
            path: &mut Vec<NodeId>,
        ) -> WorkflowResult<()> {
            states.insert(node, State::Visiting);
            path.push(node);

            for edge in graph.outgoing_edges(node) {
                match states.get(&edge.to) {
                    Some(State::Visiting) => {
                        return Err(WorkflowError::InvalidDefinition(format!(
                            "cycle detected involving node {}",
                            edge.to
                        )));
                    }
                    Some(State::Unvisited) => {
                        dfs(graph, edge.to, states, path)?;
                    }
                    _ => {}
                }
            }

            states.insert(node, State::Visited);
            path.pop();
            Ok(())
        }

        for &node in self.nodes.keys() {
            if states.get(&node) == Some(&State::Unvisited) {
                let mut path = Vec::new();
                dfs(self, node, &mut states, &mut path)?;
            }
        }

        Ok(())
    }

    /// Returns nodes in topological order.
    pub fn topological_order(&self) -> WorkflowResult<Vec<NodeId>> {
        use std::collections::VecDeque;

        let mut in_degree: HashMap<NodeId, usize> = self.nodes.keys().map(|&id| (id, 0)).collect();

        // Calculate in-degrees
        for edge in &self.edges {
            *in_degree.get_mut(&edge.to).unwrap() += 1;
        }

        // Start with nodes that have no incoming edges
        let mut queue: VecDeque<NodeId> = in_degree
            .iter()
            .filter(|(_, deg)| **deg == 0)
            .map(|(&id, _)| id)
            .collect();

        let mut result = Vec::with_capacity(self.nodes.len());

        while let Some(node) = queue.pop_front() {
            result.push(node);

            for edge in self.outgoing_edges(node) {
                let deg = in_degree.get_mut(&edge.to).unwrap();
                *deg -= 1;
                if *deg == 0 {
                    queue.push_back(edge.to);
                }
            }
        }

        if result.len() != self.nodes.len() {
            return Err(WorkflowError::InvalidDefinition(
                "cycle detected in workflow graph".into(),
            ));
        }

        Ok(result)
    }
}
