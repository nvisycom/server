//! Common data types for the nvisy-core library.
//!
//! This module provides fundamental data structures used across the nvisy ecosystem,
//! including document representation, content handling, and other shared types.
//!
//! # Overview
//!
//! The types module includes:
//!
//! - **Document**: Uniform document representation backed by efficient byte storage
//! - **Message & Chat**: Conversational AI message types and chat management
//! - **Annotation**: AI-generated annotations for content markup and analysis
//! - **Health**: Service health monitoring and status reporting
//! - **Content Types**: Standardized content type definitions for various media
//!
//! # Example
//!
//! ```rust,ignore
//! use nvisy_core::types::{Document, Message, MessageRole, Chat, Annotation, AnnotationType};
//! use bytes::Bytes;
//!
//! // Create a document
//! let doc = Document::new(Bytes::from_static(b"Hello, world!"))
//!     .with_content_type("text/plain")
//!     .with_attribute("source", "test");
//!
//! // Create a chat message
//! let message = Message::new(MessageRole::User, "What is the capital of France?")
//!     .with_token_count(10);
//!
//! // Create an annotation
//! let annotation = Annotation::new(AnnotationType::Entity, "LOCATION")
//!     .with_confidence(0.95);
//! ```

mod annotation;
mod document;
mod health;
mod message;

pub use annotation::{
    Annotation, AnnotationRelation, AnnotationSet, AnnotationType, BoundingBox, RelationType,
    TextSpan,
};
pub use document::{Document, DocumentMetadata};
pub use health::{ServiceHealth, ServiceStatus};
pub use message::{Chat, ContentPart, Message, MessageRole};

/// Common content types used throughout the nvisy ecosystem.
pub mod content_types {
    /// Plain text content type.
    pub const TEXT_PLAIN: &str = "text/plain";

    /// HTML content type.
    pub const TEXT_HTML: &str = "text/html";

    /// Markdown content type.
    pub const TEXT_MARKDOWN: &str = "text/markdown";

    /// JSON content type.
    pub const APPLICATION_JSON: &str = "application/json";

    /// PDF content type.
    pub const APPLICATION_PDF: &str = "application/pdf";

    /// JPEG image content type.
    pub const IMAGE_JPEG: &str = "image/jpeg";

    /// PNG image content type.
    pub const IMAGE_PNG: &str = "image/png";

    /// WebP image content type.
    pub const IMAGE_WEBP: &str = "image/webp";

    /// Binary/octet stream content type.
    pub const APPLICATION_OCTET_STREAM: &str = "application/octet-stream";
}

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
