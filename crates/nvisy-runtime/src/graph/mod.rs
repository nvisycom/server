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
//! - [`definition::Transformer`]: Enum of all transform definition types
//!
//! ## Compiled Types
//! Runtime-optimized types in [`compiled`]:
//! - [`compiled::CompiledGraph`]: Execution-ready graph with resolved cache slots
//! - [`compiled::CompiledNode`]: Compiled node enum (Input, Output, Transform, Switch)
//! - [`compiled::CompiledInput`], [`compiled::CompiledOutput`]: Compiled I/O nodes
//! - [`compiled::CompiledTransform`]: Compiled transform with processor structs
//! - [`compiled::Process`]: Trait for processing data in processors
//!
//! ## Compiler
//! The [`compiler`] module compiles definitions into executable graphs.

pub mod compiled;
pub mod compiler;
pub mod definition;

// Re-export commonly used types from definition module
pub use definition::{
    CacheSlot, ContentTypeCategory, ContentTypeCondition, DateField, DurationCondition, Edge,
    EdgeData, FileDateCondition, FileExtensionCondition, FileNameCondition, FileSizeCondition,
    InputDef, InputProvider, InputSource, LanguageCondition, Node, NodeCommon, NodeDef, NodeId,
    OutputDef, OutputProvider, OutputTarget, PageCountCondition, PatternMatchType, Position,
    SwitchCondition, SwitchDef, Transformer, ValidationError, WorkflowDefinition, WorkflowMetadata,
};
