//! Common data types for the nvisy-core library.
//!
//! This module provides fundamental data structures used across the nvisy ecosystem,
//! including document representation, content handling, and other shared types.

mod annotation;
mod content;
mod document;
mod health;
mod message;

pub use annotation::{
    Annotation, AnnotationRelation, AnnotationSet, AnnotationType, BoundingBox, RelationType,
    TextSpan,
};
pub use content::Content;
pub use document::{Document, DocumentId, DocumentMetadata};
pub use health::{ServiceHealth, ServiceStatus};
pub use message::{Chat, Message, MessageRole};

/// Result type alias for operations in the types module.
pub type Result<T, E = TypeError> = std::result::Result<T, E>;

/// Error type for type-related operations.
#[derive(Debug, thiserror::Error)]
pub enum TypeError {
    /// Invalid content type format.
    #[error("Invalid content type: {0}")]
    InvalidContentType(String),

    /// Invalid metadata value.
    #[error("Invalid metadata value for key '{key}': {reason}")]
    InvalidMetadata { key: String, reason: String },

    /// Document validation failed.
    #[error("Document validation failed: {0}")]
    ValidationFailed(String),

    /// Serialization/deserialization error.
    #[error("Serialization error: {0}")]
    SerializationError(String),
}
