//! Workflow graph structures and node types.
//!
//! This module provides the graph representation for workflows:
//!
//! ## Definition Types
//! Serializable, frontend-friendly types in [`definition`]:
//! - [`definition::WorkflowDefinition`]: JSON-serializable workflow structure
//! - [`definition::NodeDef`]: Node definition enum (Input, Transform, Output, Switch)
//! - [`definition::InputDef`], [`definition::OutputDef`]: I/O node definitions
//! - [`definition::CacheSlot`]: Named cache slot for inter-node data passing
//!
//! ## Compiled Types
//! Runtime-optimized types in [`compiled`]:
//! - [`compiled::CompiledGraph`]: Execution-ready graph with resolved cache slots
//! - [`compiled::CompiledNode`]: Compiled node enum (Input, Output, Transform, Switch)
//! - [`compiled::CompiledInput`], [`compiled::CompiledOutput`]: Compiled I/O nodes
//! - [`compiled::CompiledTransform`]: Compiled transform with processor structs
//!
//! ## Transform Types
//! Transform definitions in [`transform`]:
//! - [`transform::Transformer`]: Enum of all transform types
//! - [`transform::Transform`]: Trait for data transformation
//!
//! ## Compiler
//! The [`compiler`] module compiles definitions into executable graphs.

pub mod compiled;
pub mod compiler;
pub mod definition;
pub mod transform;

// Re-export commonly used types from definition module
pub use definition::{
    CacheSlot, Edge, EdgeData, InputDef, InputProvider, InputSource, Node, NodeCommon, NodeDef,
    NodeId, OutputDef, OutputProviderDef, OutputTarget, SwitchBranch, SwitchCondition, SwitchDef,
    ValidationError, WorkflowDefinition, WorkflowMetadata,
};

// Re-export transform types
pub use transform::Transformer;
