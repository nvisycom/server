//! Convenience re-exports for common types and traits.
//!
//! This prelude module re-exports the most commonly used types from this crate,
//! making it easy to import everything needed for typical Qdrant operations.

pub use crate::client::{QdrantClient, QdrantConfig};
pub use crate::collection::{
    AnnotationCollection, ConversationCollection, DocumentCollection, SearchParams,
};
pub use crate::error::{Error, Result};
pub use crate::payload::{
    AnnotationPoint, AnnotationType, ConversationPoint, ConversationStatus, DocumentPoint,
    DocumentStatus, DocumentType, MessageType,
};
pub use crate::types::{Distance, Payload, Point, PointId, Vector, VectorParams};
