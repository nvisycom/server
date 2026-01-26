//! Context types for data operations.
//!
//! Contexts carry state from previous runs to enable pagination and resumption.

use derive_more::From;
use serde::{Deserialize, Serialize};

/// Context for object storage operations (S3, GCS, Azure Blob).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ObjectContext {
    /// Path prefix for listing objects.
    pub prefix: Option<String>,
    /// Continuation token for pagination.
    pub token: Option<String>,
    /// Maximum number of items to read.
    pub limit: Option<usize>,
}

/// Context for relational database operations (Postgres, MySQL).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RelationalContext {
    /// Last seen cursor value (for keyset pagination).
    pub cursor: Option<String>,
    /// Tiebreaker value for resolving cursor conflicts.
    pub tiebreaker: Option<String>,
    /// Maximum number of items to read.
    pub limit: Option<usize>,
}

/// Context for vector database operations (Qdrant, Pinecone, pgvector).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VectorContext {
    /// Continuation token or offset for pagination.
    pub token: Option<String>,
    /// Maximum number of items to read.
    pub limit: Option<usize>,
}

/// Type-erased context for runtime dispatch.
#[derive(Debug, Clone, Default, From, Serialize, Deserialize)]
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
    /// Returns the limit if set in any context type.
    pub fn limit(&self) -> Option<usize> {
        match self {
            Self::None => None,
            Self::Object(ctx) => ctx.limit,
            Self::Relational(ctx) => ctx.limit,
            Self::Vector(ctx) => ctx.limit,
        }
    }

    /// Sets the limit on the inner context.
    pub fn with_limit(mut self, limit: usize) -> Self {
        match &mut self {
            Self::None => {}
            Self::Object(ctx) => ctx.limit = Some(limit),
            Self::Relational(ctx) => ctx.limit = Some(limit),
            Self::Vector(ctx) => ctx.limit = Some(limit),
        }
        self
    }
}
