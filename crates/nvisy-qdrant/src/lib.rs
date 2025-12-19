#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

/// Tracing target for Qdrant client operations.
///
/// Use this target for logging client initialization, configuration, and client-level errors.
pub const TRACING_TARGET_CLIENT: &str = "nvisy_qdrant::client";

/// Tracing target for Qdrant collection operations.
///
/// Use this target for logging collection creation, deletion, configuration, and collection-related errors.
pub const TRACING_TARGET_COLLECTIONS: &str = "nvisy_qdrant::collections";

/// Tracing target for Qdrant point operations.
///
/// Use this target for logging point CRUD operations, batch operations, and point-related errors.
pub const TRACING_TARGET_POINTS: &str = "nvisy_qdrant::points";

/// Tracing target for Qdrant search operations.
///
/// Use this target for logging vector searches, filtering, scoring, and search-related errors.
pub const TRACING_TARGET_SEARCH: &str = "nvisy_qdrant::search";

/// Tracing target for Qdrant connection operations.
///
/// Use this target for logging connection establishment, health checks, and connection errors.
pub const TRACING_TARGET_CONNECTION: &str = "nvisy_qdrant::connection";

mod client;
pub mod collection;
mod error;
pub mod payload;
#[doc(hidden)]
pub mod prelude;
pub mod types;

pub use client::{QdrantClient, QdrantConfig, QdrantConnection};
pub use collection::{
    AnnotationCollection, AnnotationConfig, AuthorStats, ConversationCollection,
    ConversationConfig, ConversationStats, DocumentCollection, DocumentConfig, DocumentStats,
    DocumentTypeStats, SearchParams as CollectionSearchParams,
};
pub use error::{QdrantError, QdrantResult};
pub use payload::{
    AnnotationCoordinates, AnnotationPoint, AnnotationType, ConversationPoint, ConversationStatus,
    DocumentPoint, DocumentStatus, DocumentType, MessageType,
};
pub use types::{
    CollectionConfig, CollectionInfo, CollectionStatus, Distance, Payload, Point, PointId, Vector,
    VectorParams,
};
