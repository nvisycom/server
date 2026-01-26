//! Compiled node types.

use derive_more::From;
use nvisy_dal::datatypes::AnyDataValue;
use nvisy_dal::streams;

use super::route::CompiledSwitch;
use super::transform::CompiledTransform;

/// Type alias for input streams in the runtime.
pub type InputStream = streams::InputStream<AnyDataValue>;

/// Type alias for output streams in the runtime.
pub type OutputStream = streams::OutputStream<AnyDataValue>;

/// Compiled node enum for workflow execution.
///
/// This is the runtime representation of a node after compilation.
/// Cache slots are resolved during compilation, so compiled nodes
/// only contain concrete processing types.
#[derive(Debug, From)]
pub enum CompiledNode {
    /// Data input node - ready to stream data.
    Input(InputStream),
    /// Data output node - ready to receive data.
    Output(OutputStream),
    /// Data transform node - ready to process data.
    /// Boxed to reduce enum size variance (transform processors are large).
    Transform(Box<CompiledTransform>),
    /// Conditional routing node - evaluates conditions.
    Switch(CompiledSwitch),
}

impl CompiledNode {
    /// Returns whether this is an input node.
    pub const fn is_input(&self) -> bool {
        matches!(self, CompiledNode::Input(_))
    }

    /// Returns whether this is an output node.
    pub const fn is_output(&self) -> bool {
        matches!(self, CompiledNode::Output(_))
    }

    /// Returns whether this is a transform node.
    pub const fn is_transform(&self) -> bool {
        matches!(self, CompiledNode::Transform(_))
    }

    /// Returns whether this is a switch node.
    pub const fn is_switch(&self) -> bool {
        matches!(self, CompiledNode::Switch(_))
    }

    /// Returns this node as a transform, if it is one.
    pub fn as_transform(&self) -> Option<&CompiledTransform> {
        match self {
            CompiledNode::Transform(transform) => Some(transform.as_ref()),
            _ => None,
        }
    }

    /// Returns this node as a switch, if it is one.
    pub fn as_switch(&self) -> Option<&CompiledSwitch> {
        match self {
            CompiledNode::Switch(switch) => Some(switch),
            _ => None,
        }
    }
}

impl From<CompiledTransform> for CompiledNode {
    fn from(transform: CompiledTransform) -> Self {
        CompiledNode::Transform(Box::new(transform))
    }
}
