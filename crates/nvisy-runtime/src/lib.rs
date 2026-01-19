//! Document processing runtime for nvisy.
//!
//! This crate provides a service wrapper around the nvisy runtime engine,
//! integrating document processing capabilities with the server infrastructure.

mod archive;
mod service;

pub use nvisy_rt_core as rt_core;
pub use nvisy_rt_engine as rt_engine;

pub use archive::{ArchiveError, ArchiveFormat, ArchiveResult, ArchiveService};
pub use service::{RuntimeConfig, RuntimeService};

// Re-export commonly used types from the engine
pub use nvisy_rt_engine::{
    BoundingBox, Capabilities, Document, DocumentFormat, Engine, EngineConfig, FormatRegistry,
    LoadedDocument, Point, Region, RegionId, RegionKind, doc,
};
