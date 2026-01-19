//! Runtime services for document processing.

mod config;
mod service;

pub use config::RuntimeConfig;
pub use service::RuntimeService;

// Re-export commonly used types from the runtime crates
pub use nvisy_rt_core as rt_core;
pub use nvisy_rt_engine as rt_engine;

pub use nvisy_rt_engine::{
    BoundingBox, Capabilities, Document, DocumentFormat, Engine, EngineConfig, FormatRegistry,
    LoadedDocument, Point, Region, RegionId, RegionKind, doc,
};
