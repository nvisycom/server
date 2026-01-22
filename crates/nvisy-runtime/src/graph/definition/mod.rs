//! Workflow definition types.
//!
//! This module contains serializable, frontend-friendly types for defining workflows.
//! These types are designed for:
//! - Easy serialization to/from JSON
//! - Frontend consumption and editing
//! - Storage in databases
//!
//! To execute a workflow, definitions must be compiled into runtime types
//! using the [`crate::graph::compiler`] module.

mod edge;
mod input;
mod metadata;
mod node;
mod output;
mod route;
mod transform;
mod workflow;

pub use edge::{Edge, EdgeData};
pub use input::{InputDef, InputProvider, InputSource};
pub use metadata::WorkflowMetadata;
pub use node::{Node, NodeCommon, NodeDef, NodeId};
pub use output::{OutputDef, OutputProviderDef, OutputTarget};
pub use route::{
    CacheSlot, ContentTypeCategory, DateField, SwitchBranch, SwitchCondition, SwitchDef,
};
pub use transform::{Chunk, Derive, Embedding, Enrich, Extract, Partition, Transform, Transformer};
pub use workflow::{ValidationError, WorkflowDefinition};
