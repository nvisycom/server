//! Graph data type with nodes and edges.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::DataType;

/// A graph containing nodes and edges.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Graph {
    /// Nodes in the graph.
    #[serde(default)]
    pub nodes: Vec<Node>,
    /// Edges in the graph.
    #[serde(default)]
    pub edges: Vec<Edge>,
}

impl Graph {
    /// Creates a new empty graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a node.
    pub fn with_node(mut self, node: Node) -> Self {
        self.nodes.push(node);
        self
    }

    /// Adds an edge.
    pub fn with_edge(mut self, edge: Edge) -> Self {
        self.edges.push(edge);
        self
    }
}

impl DataType for Graph {
    const TYPE_ID: &'static str = "graph";

    fn data_type_id() -> super::DataTypeId {
        super::DataTypeId::Graph
    }
}

/// A node in a graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    /// Unique identifier.
    pub id: String,
    /// Node labels (types).
    #[serde(default)]
    pub labels: Vec<String>,
    /// Node properties.
    #[serde(default)]
    pub properties: HashMap<String, serde_json::Value>,
}

impl Node {
    /// Creates a new node.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            labels: Vec::new(),
            properties: HashMap::new(),
        }
    }

    /// Adds a label.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.labels.push(label.into());
        self
    }

    /// Sets a property.
    pub fn with_property(
        mut self,
        key: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Self {
        self.properties.insert(key.into(), value.into());
        self
    }
}

/// An edge in a graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    /// Unique identifier.
    pub id: String,
    /// Source node ID.
    pub from: String,
    /// Target node ID.
    pub to: String,
    /// Edge label (relationship type).
    pub label: String,
    /// Edge properties.
    #[serde(default)]
    pub properties: HashMap<String, serde_json::Value>,
}

impl Edge {
    /// Creates a new edge.
    pub fn new(
        id: impl Into<String>,
        from: impl Into<String>,
        to: impl Into<String>,
        label: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            from: from.into(),
            to: to.into(),
            label: label.into(),
            properties: HashMap::new(),
        }
    }

    /// Sets a property.
    pub fn with_property(
        mut self,
        key: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Self {
        self.properties.insert(key.into(), value.into());
        self
    }
}
