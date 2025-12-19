#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

//! Core abstractions and shared types for AI services in the Nvisy ecosystem.

pub mod emb;
mod error;
pub mod ocr;
pub mod types;
pub mod vlm;

// Re-export key types for convenience
pub use error::BoxedError;
// Re-export core types from the types module
pub use types::{
    // Annotation types
    Annotation,
    AnnotationRelation,
    AnnotationSet,
    AnnotationType,
    BoundingBox,
    // Message and chat types
    Chat,
    ContentPart,

    // Document types
    Document,
    DocumentMetadata,

    Message,
    MessageRole,
    RelationType,

    // Health monitoring types
    ServiceHealth,
    ServiceStatus,
    TextSpan,
};
