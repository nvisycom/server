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
pub mod collections;
mod error;
mod manager;
pub mod payload;
#[doc(hidden)]
pub mod prelude;
mod search;
pub mod types;

pub use client::{QdrantClient, QdrantConfig, QdrantConnection};
// Re-export collection operation traits
pub use collections::{
    AnnotationConfig, AnnotationOperations, AuthorStats, ConversationConfig,
    ConversationOperations, ConversationStats, DocumentConfig, DocumentOperations, DocumentStats,
    DocumentTypeStats, SearchParams as CollectionSearchParams,
};
pub use error::{QdrantError, QdrantResult};
// Re-export payload types for easy access
pub use payload::{
    AnnotationCoordinates, AnnotationPoint, AnnotationType, ConversationPoint, ConversationStatus,
    DocumentPoint, DocumentStatus, DocumentType, MessageType,
};
// Re-export the main Qdrant client for advanced usage
pub use qdrant_client::Qdrant as RawQdrantClient;
// Re-export commonly used Qdrant client types for convenience
pub use qdrant_client::qdrant::{
    Condition, FieldCondition, Filter, Match, Range, SearchPoints, WithPayloadSelector,
    condition::ConditionOneOf, r#match::MatchValue, point_id::PointIdOptions,
    vectors::VectorsOptions, with_payload_selector::SelectorOptions,
};
pub use search::{
    BatchSearchRequest, BatchSearchResults, SearchParams, SearchResult, SearchResults,
};
pub use types::{
    CollectionConfig, CollectionInfo, CollectionStatus, Distance, Payload, Point, PointId, Vector,
    VectorParams,
};
