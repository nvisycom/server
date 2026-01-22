//! Generic node wrapper, node identifier, and node data types.

use std::str::FromStr;

use derive_more::{Debug, Display, From, Into};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::graph::input::InputNode;
use crate::graph::output::OutputNode;
use crate::graph::transform::TransformerConfig;

/// Unique identifier for a node in a workflow graph.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[derive(Debug, Display, From, Into)]
#[debug("{_0}")]
#[display("{_0}")]
#[serde(transparent)]
pub struct NodeId(Uuid);

impl NodeId {
    /// Creates a new random node ID.
    #[inline]
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    /// Creates a node ID from an existing UUID.
    #[inline]
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Returns the underlying UUID.
    #[inline]
    pub const fn as_uuid(&self) -> Uuid {
        self.0
    }

    /// Returns the UUID as bytes.
    #[inline]
    pub const fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }
}

impl Default for NodeId {
    fn default() -> Self {
        Self::new()
    }
}

impl FromStr for NodeId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::from_str(s)?))
    }
}

impl AsRef<Uuid> for NodeId {
    fn as_ref(&self) -> &Uuid {
        &self.0
    }
}

/// A generic node wrapper that adds optional name and description to any inner type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeCommon<T> {
    /// Display name of the node.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Description of what this node does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Inner node configuration.
    #[serde(flatten)]
    pub inner: T,
}

impl<T> NodeCommon<T> {
    /// Creates a new node with the given inner value.
    pub fn new(inner: T) -> Self {
        Self {
            name: None,
            description: None,
            inner,
        }
    }

    /// Sets the display name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Sets the description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// A workflow node with id, name, description, and node data.
pub type Node = NodeCommon<NodeData>;

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
    Transformer(TransformerConfig),
    /// Data output node, writes or consumes data.
    Output(OutputNode),
}

impl NodeData {
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
