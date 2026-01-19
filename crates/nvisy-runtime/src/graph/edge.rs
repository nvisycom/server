//! Edge types for connecting nodes in a workflow graph.

use serde::{Deserialize, Serialize};

use crate::node::NodeId;

/// An edge connecting two nodes in the workflow graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Edge {
    /// Source node ID.
    pub from: NodeId,
    /// Target node ID.
    pub to: NodeId,
    /// Optional port/slot name on the source node.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_port: Option<String>,
    /// Optional port/slot name on the target node.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_port: Option<String>,
}

impl Edge {
    /// Creates a new edge between two nodes.
    pub fn new(from: NodeId, to: NodeId) -> Self {
        Self {
            from,
            to,
            from_port: None,
            to_port: None,
        }
    }

    /// Creates an edge with port specifications.
    pub fn with_ports(
        from: NodeId,
        from_port: impl Into<String>,
        to: NodeId,
        to_port: impl Into<String>,
    ) -> Self {
        Self {
            from,
            to,
            from_port: Some(from_port.into()),
            to_port: Some(to_port.into()),
        }
    }

    /// Sets the source port.
    pub fn from_port(mut self, port: impl Into<String>) -> Self {
        self.from_port = Some(port.into());
        self
    }

    /// Sets the target port.
    pub fn to_port(mut self, port: impl Into<String>) -> Self {
        self.to_port = Some(port.into());
        self
    }
}
