//! Prelude module for convenient imports.
//!
//! This module re-exports commonly used types for ergonomic imports:
//!
//! ```rust
//! use nvisy_workflow::prelude::*;
//! ```

pub use crate::engine::{Engine, EngineConfig};
pub use crate::error::{WorkflowError, WorkflowResult};
pub use crate::graph::{Edge, WorkflowGraph};
pub use crate::node::{
    NodeData, NodeId, SinkKind, SinkNode, SourceKind, SourceNode, TransformerKind, TransformerNode,
};
