//! Compiled node types.

use super::input::CompiledInput;
use super::output::CompiledOutput;
use super::route::CompiledSwitch;
use super::transform::CompiledTransform;

/// Compiled node enum for workflow execution.
///
/// This is the runtime representation of a node after compilation.
/// Cache slots are resolved during compilation, so compiled nodes
/// only contain concrete processing types.
#[derive(Debug)]
pub enum CompiledNode {
    /// Data input node - ready to stream data.
    Input(CompiledInput),
    /// Data output node - ready to receive data.
    Output(CompiledOutput),
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

    /// Returns this node as an input, if it is one.
    pub fn as_input(&self) -> Option<&CompiledInput> {
        match self {
            CompiledNode::Input(input) => Some(input),
            _ => None,
        }
    }

    /// Returns this node as an output, if it is one.
    pub fn as_output(&self) -> Option<&CompiledOutput> {
        match self {
            CompiledNode::Output(output) => Some(output),
            _ => None,
        }
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

    /// Consumes this node and returns the input, if it is one.
    pub fn into_input(self) -> Option<CompiledInput> {
        match self {
            CompiledNode::Input(input) => Some(input),
            _ => None,
        }
    }

    /// Consumes this node and returns the output, if it is one.
    pub fn into_output(self) -> Option<CompiledOutput> {
        match self {
            CompiledNode::Output(output) => Some(output),
            _ => None,
        }
    }

    /// Consumes this node and returns the transform, if it is one.
    pub fn into_transform(self) -> Option<Box<CompiledTransform>> {
        match self {
            CompiledNode::Transform(transform) => Some(transform),
            _ => None,
        }
    }

    /// Consumes this node and returns the switch, if it is one.
    pub fn into_switch(self) -> Option<CompiledSwitch> {
        match self {
            CompiledNode::Switch(switch) => Some(switch),
            _ => None,
        }
    }
}

impl From<CompiledInput> for CompiledNode {
    fn from(input: CompiledInput) -> Self {
        CompiledNode::Input(input)
    }
}

impl From<CompiledOutput> for CompiledNode {
    fn from(output: CompiledOutput) -> Self {
        CompiledNode::Output(output)
    }
}

impl From<CompiledTransform> for CompiledNode {
    fn from(transform: CompiledTransform) -> Self {
        CompiledNode::Transform(Box::new(transform))
    }
}

impl From<Box<CompiledTransform>> for CompiledNode {
    fn from(transform: Box<CompiledTransform>) -> Self {
        CompiledNode::Transform(transform)
    }
}

impl From<CompiledSwitch> for CompiledNode {
    fn from(switch: CompiledSwitch) -> Self {
        CompiledNode::Switch(switch)
    }
}
