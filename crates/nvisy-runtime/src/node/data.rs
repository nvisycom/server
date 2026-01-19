//! Core node data enum.

use derive_more::From;
use serde::{Deserialize, Serialize};

use super::input::InputNode;
use super::output::OutputNode;
use super::transformer::TransformerNode;

/// Data associated with a workflow node.
///
/// Nodes are categorized by their role in data flow:
/// - **Input**: Reads/produces data (entry points)
/// - **Transformer**: Processes/transforms data (intermediate)
/// - **Output**: Writes/consumes data (exit points)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, From)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NodeData {
    /// Data input node, reads or produces data.
    Input(InputNode),
    /// Data transformer node, processes or transforms data.
    Transformer(TransformerNode),
    /// Data output node, writes or consumes data.
    Output(OutputNode),
}

impl NodeData {
    /// Returns the node's display name if set.
    pub fn name(&self) -> Option<&str> {
        match self {
            NodeData::Input(n) => n.name.as_deref(),
            NodeData::Transformer(n) => n.name.as_deref(),
            NodeData::Output(n) => n.name.as_deref(),
        }
    }

    /// Returns the node's description if set.
    pub fn description(&self) -> Option<&str> {
        match self {
            NodeData::Input(n) => n.description.as_deref(),
            NodeData::Transformer(n) => n.description.as_deref(),
            NodeData::Output(n) => n.description.as_deref(),
        }
    }

    /// Returns whether this is an input node.
    pub const fn is_input(&self) -> bool {
        matches!(self, NodeData::Input(_))
    }

    /// Returns whether this is a transformer node.
    pub const fn is_transformer(&self) -> bool {
        matches!(self, NodeData::Transformer(_))
    }

    /// Returns whether this is an output node.
    pub const fn is_output(&self) -> bool {
        matches!(self, NodeData::Output(_))
    }
}
