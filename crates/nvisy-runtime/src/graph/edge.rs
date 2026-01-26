//! Edge data for compiled graphs.

use serde::{Deserialize, Serialize};

/// Edge data stored in the compiled graph.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EdgeData {
    /// Optional port/slot name on the source node.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_port: Option<String>,
    /// Optional port/slot name on the target node.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_port: Option<String>,
}
