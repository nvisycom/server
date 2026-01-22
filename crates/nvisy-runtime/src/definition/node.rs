//! Node definition types.

use std::str::FromStr;

use derive_builder::Builder;
use derive_more::{Debug, Display, From, Into};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::input::Input;
use super::output::Output;
use super::route::SwitchDef;
use super::transform::Transformer;
use super::util::Position;

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

/// A workflow node definition with metadata and kind.
///
/// Nodes are categorized by their role in data flow:
/// - **Input**: Reads/produces data (entry points)
/// - **Transform**: Processes/transforms data (intermediate)
/// - **Output**: Writes/consumes data (exit points)
/// - **Switch**: Routes data based on conditions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Builder)]
#[builder(
    name = "NodeBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with")
)]
pub struct Node {
    /// Display name of the node.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub name: Option<String>,
    /// Description of what this node does.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub description: Option<String>,
    /// Position in the visual editor.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub position: Option<Position>,
    /// The node kind/type.
    #[serde(flatten)]
    pub kind: NodeKind,
}

impl Node {
    /// Creates a new node with the given kind.
    pub fn new(kind: impl Into<NodeKind>) -> Self {
        Self {
            name: None,
            description: None,
            position: None,
            kind: kind.into(),
        }
    }

    /// Returns a builder for creating a node.
    pub fn builder() -> NodeBuilder {
        NodeBuilder::default()
    }

    /// Returns whether this is an input node.
    pub const fn is_input(&self) -> bool {
        self.kind.is_input()
    }

    /// Returns whether this is a transform node.
    pub const fn is_transform(&self) -> bool {
        self.kind.is_transform()
    }

    /// Returns whether this is an output node.
    pub const fn is_output(&self) -> bool {
        self.kind.is_output()
    }

    /// Returns whether this is a switch node.
    pub const fn is_switch(&self) -> bool {
        self.kind.is_switch()
    }
}

/// Node kind enum for workflow graphs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, From)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NodeKind {
    /// Data input node, reads or produces data.
    Input(Input),
    /// Data transformer node, processes or transforms data.
    Transform(Transformer),
    /// Data output node, writes or consumes data.
    Output(Output),
    /// Conditional routing node.
    Switch(SwitchDef),
}

impl NodeKind {
    /// Returns whether this is an input node.
    pub const fn is_input(&self) -> bool {
        matches!(self, NodeKind::Input(_))
    }

    /// Returns whether this is a transform node.
    pub const fn is_transform(&self) -> bool {
        matches!(self, NodeKind::Transform(_))
    }

    /// Returns whether this is an output node.
    pub const fn is_output(&self) -> bool {
        matches!(self, NodeKind::Output(_))
    }

    /// Returns whether this is a switch node.
    pub const fn is_switch(&self) -> bool {
        matches!(self, NodeKind::Switch(_))
    }
}
