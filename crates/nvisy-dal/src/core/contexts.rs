//! Context types for data operations.
//!
//! Contexts carry state needed to resume reading from a specific position.
//! They only track *where* to resume, not *how much* to read (that's in Params).

use derive_more::From;
use serde::{Deserialize, Serialize};

/// Context for object storage operations (S3, GCS, Azure Blob).
///
/// Uses marker-based pagination (last seen key) which is portable across
/// S3, GCS, Azure Blob, and MinIO.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectContext {
    /// Path prefix for listing objects.
    pub prefix: Option<String>,
    /// Last seen object key (used as StartAfter/marker for resumption).
    pub token: Option<String>,
}

/// Context for relational database operations (Postgres, MySQL).
///
/// Uses keyset pagination which is more efficient than offset-based
/// pagination for large datasets and provides stable results.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationalContext {
    /// Last seen cursor value (for keyset pagination).
    pub cursor: Option<String>,
    /// Tiebreaker value for resolving cursor conflicts.
    pub tiebreaker: Option<String>,
}

/// Context for vector database operations (Qdrant, Pinecone, pgvector).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VectorContext {
    /// Continuation token or offset for pagination.
    pub token: Option<String>,
}

/// Type-erased context for runtime dispatch.
#[derive(Debug, Clone, Default, PartialEq, Eq, From, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum AnyContext {
    /// No context / empty state.
    #[default]
    None,
    /// Object storage context.
    Object(ObjectContext),
    /// Relational database context.
    Relational(RelationalContext),
    /// Vector database context.
    Vector(VectorContext),
}

impl AnyContext {
    /// Returns a reference to the object context if this is an object context.
    pub fn as_object(&self) -> Option<&ObjectContext> {
        match self {
            Self::Object(ctx) => Some(ctx),
            _ => None,
        }
    }

    /// Returns a reference to the relational context if this is a relational context.
    pub fn as_relational(&self) -> Option<&RelationalContext> {
        match self {
            Self::Relational(ctx) => Some(ctx),
            _ => None,
        }
    }

    /// Returns a reference to the vector context if this is a vector context.
    pub fn as_vector(&self) -> Option<&VectorContext> {
        match self {
            Self::Vector(ctx) => Some(ctx),
            _ => None,
        }
    }
}
