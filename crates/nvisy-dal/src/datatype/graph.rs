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

impl DataType for Graph {}

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
