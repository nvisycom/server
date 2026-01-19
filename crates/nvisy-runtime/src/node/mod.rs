//! Node types for workflow graphs.
//!
//! This module provides the core node abstractions:
//! - [`NodeId`]: Unique identifier for nodes
//! - [`NodeData`]: Data associated with each node (Input, Transformer, Output)

mod data;
mod id;
pub mod input;
pub mod output;
pub mod transformer;

pub use data::NodeData;
pub use id::NodeId;
pub use input::{InputConfig, InputNode};
pub use output::{OutputConfig, OutputNode};
pub use transformer::{TransformerConfig, TransformerNode};
