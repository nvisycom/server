//! Convenience re-exports for common types and traits.
//!
//! This prelude module re-exports the most commonly used types from this crate,
//! making it easy to import everything needed for typical Qdrant operations.

pub use crate::client::{QdrantClient, QdrantConfig, QdrantConnection};
pub use crate::collection::{
    AnnotationCollection, AnnotationConfig, ConversationCollection, ConversationConfig,
    DocumentCollection, DocumentConfig,
};
pub use crate::error::{QdrantError, QdrantResult};
pub use crate::types::{
    CollectionConfig, CollectionInfo, CollectionStatus, Distance, Payload, Point, PointId, Vector,
    VectorParams,
};
