//! Edge types for connecting nodes in a workflow graph.

use derive_builder::Builder;
use serde::{Deserialize, Serialize};

use super::NodeId;

/// An edge connecting two nodes in the workflow graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Builder)]
#[builder(
    name = "EdgeBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(validate = "Self::validate")
)]
pub struct Edge {
    /// Source node ID.
    pub from: NodeId,
    /// Target node ID.
    pub to: NodeId,
    /// Optional port/slot name on the source node.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub from_port: Option<String>,
    /// Optional port/slot name on the target node.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub to_port: Option<String>,
}

impl EdgeBuilder {
    fn validate(&self) -> Result<(), String> {
        if self.from.is_none() {
            return Err("from is required".into());
        }
        if self.to.is_none() {
            return Err("to is required".into());
        }
        Ok(())
    }
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

    /// Returns a builder for creating an edge.
    pub fn builder() -> EdgeBuilder {
        EdgeBuilder::default()
    }
}
