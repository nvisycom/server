//! Edge types for connecting nodes in a workflow graph.

use serde::{Deserialize, Serialize};

use super::NodeId;

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
