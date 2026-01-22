//! Edge data for compiled graphs.

use derive_builder::Builder;
use serde::{Deserialize, Serialize};

/// Edge data stored in the compiled graph.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, Builder)]
#[builder(
    name = "EdgeDataBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with")
)]
pub struct EdgeData {
    /// Optional port/slot name on the source node.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub from_port: Option<String>,
    /// Optional port/slot name on the target node.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub to_port: Option<String>,
}

impl EdgeData {
    /// Returns a builder for creating edge data.
    pub fn builder() -> EdgeDataBuilder {
        EdgeDataBuilder::default()
    }
}
