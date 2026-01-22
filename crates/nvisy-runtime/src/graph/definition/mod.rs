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
pub use node::{Node, NodeCommon, NodeDef, NodeId, Position};
pub use output::{OutputDef, OutputProvider, OutputTarget};
pub use route::{
    CacheSlot, ContentTypeCategory, ContentTypeCondition, DateField, DurationCondition,
    FileDateCondition, FileExtensionCondition, FileNameCondition, FileSizeCondition,
    LanguageCondition, PageCountCondition, PatternMatchType, SwitchCondition, SwitchDef,
};
pub use transform::{
    AnalyzeTask, Chunk, ChunkStrategy, ConvertTask, Derive, DeriveTask, Embedding, Enrich,
    EnrichTask, Extract, ExtractTask, ImageEnrichTask, Partition, PartitionStrategy,
    TableConvertTask, TableEnrichTask, TextConvertTask, Transformer,
};
pub use workflow::{ValidationError, WorkflowDefinition};
