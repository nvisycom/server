//! Core types for inference operations.
//!
//! This module provides foundational types used across inference operations:
//! - [`Annotation`] - AI-generated markup and insights
//! - [`Content`] - Unified content representation (text, documents, chat)
//! - [`Document`] - Binary document with metadata
//! - [`Message`] - Chat message with role

mod annotation;
mod content;
mod document;
mod message;

pub use annotation::{
    Annotation, AnnotationRelation, AnnotationSet, AnnotationType, BoundingBox, RelationType,
    TextSpan,
};
pub use content::Content;
pub use document::{Document, DocumentId, DocumentMetadata};
pub use message::{Chat, Message, MessageRole};
pub use nvisy_core::types::{ServiceHealth, ServiceStatus, Timing};
