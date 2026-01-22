//! Node definition types.

use std::str::FromStr;

use derive_builder::Builder;
use derive_more::{Debug, Display, From, Into};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::input::InputDef;
use super::output::OutputDef;
use super::route::SwitchDef;
use super::transform::Transformer;

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

/// Position of a node in the visual editor.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Position {
    /// X coordinate.
    pub x: f32,
    /// Y coordinate.
    pub y: f32,
}

impl Position {
    /// Creates a new position.
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// A generic node wrapper that adds optional name and description to any inner type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Builder)]
#[builder(
    name = "NodeCommonBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(validate = "Self::validate")
)]
pub struct NodeCommon<T>
where
    T: Clone,
{
    /// Display name of the node.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub name: Option<String>,
    /// Description of what this node does.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub description: Option<String>,
    /// Position in the visual editor.
    #[serde(default, skip_serializing_if = "is_default_position")]
    #[builder(default)]
    pub position: Position,
    /// Inner node configuration.
    #[serde(flatten)]
    pub inner: T,
}

fn is_default_position(pos: &Position) -> bool {
    pos.x == 0.0 && pos.y == 0.0
}

impl<T: Clone> NodeCommonBuilder<T> {
    fn validate(&self) -> Result<(), String> {
        if self.inner.is_none() {
            return Err("inner is required".into());
        }
        Ok(())
    }
}

impl<T: Clone> NodeCommon<T> {
    /// Creates a new node with the given inner value.
    pub fn new(inner: T) -> Self {
        Self {
            name: None,
            description: None,
            position: Position::default(),
            inner,
        }
    }

    /// Returns a builder for creating a node.
    pub fn builder() -> NodeCommonBuilder<T> {
        NodeCommonBuilder::default()
    }
}

/// A workflow node definition with common metadata.
pub type Node = NodeCommon<NodeDef>;

/// Node definition enum for workflow graphs.
///
/// Nodes are categorized by their role in data flow:
/// - **Input**: Reads/produces data (entry points)
/// - **Transform**: Processes/transforms data (intermediate)
/// - **Output**: Writes/consumes data (exit points)
/// - **Switch**: Routes data based on conditions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, From)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NodeDef {
    /// Data input node, reads or produces data.
    Input(InputDef),
    /// Data transformer node, processes or transforms data.
    Transform(Transformer),
    /// Data output node, writes or consumes data.
    Output(OutputDef),
    /// Conditional routing node.
    Switch(SwitchDef),
}

impl NodeDef {
    /// Returns whether this is an input node.
    pub const fn is_input(&self) -> bool {
        matches!(self, NodeDef::Input(_))
    }

    /// Returns whether this is a transform node.
    pub const fn is_transform(&self) -> bool {
        matches!(self, NodeDef::Transform(_))
    }

    /// Returns whether this is an output node.
    pub const fn is_output(&self) -> bool {
        matches!(self, NodeDef::Output(_))
    }

    /// Returns whether this is a switch node.
    pub const fn is_switch(&self) -> bool {
        matches!(self, NodeDef::Switch(_))
    }
}
