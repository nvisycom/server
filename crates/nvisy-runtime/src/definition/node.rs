//! Node definition types.

use std::str::FromStr;

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

/// A workflow node definition with metadata and kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    /// Display name of the node.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Description of what this node does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Position in the visual editor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    /// The node kind/type.
    #[serde(flatten)]
    pub kind: NodeKind,
}

impl Node {
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
