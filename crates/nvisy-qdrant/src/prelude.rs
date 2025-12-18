//! Convenience re-exports for common types and traits.
//!
//! This prelude module re-exports the most commonly used types from this crate,
//! making it easy to import everything needed for typical Qdrant operations.

pub use crate::client::{QdrantClient, QdrantConfig, QdrantConnection};
pub use crate::collections::{
    AnnotationConfig, AnnotationOperations, ConversationConfig, ConversationOperations,
    DocumentConfig, DocumentOperations,
};
pub use crate::error::{QdrantError, QdrantResult};
pub use crate::search::{
    BatchSearchRequest, BatchSearchResults, SearchBuilder, SearchParams as SearchParameters,
    SearchRequest, SearchResult, SearchResults,
};
pub use crate::types::{
    CollectionConfig, CollectionInfo, CollectionStatus, Distance, Payload, Point, PointId, Vector,
    VectorParams,
};
