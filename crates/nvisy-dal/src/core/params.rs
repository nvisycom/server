//! Parameter types for provider configuration.
//!
//! Params define how providers operate (columns, batch sizes, etc.),
//! while contexts carry runtime state (cursors, tokens, limits).

use serde::{Deserialize, Serialize};

/// Common parameters for relational database operations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RelationalParams {
    /// Target table name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table: Option<String>,
    /// Column to use for cursor-based pagination (e.g., "id", "created_at").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor_column: Option<String>,
    /// Column to use as tiebreaker when cursor values are not unique (e.g., "id").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tiebreaker_column: Option<String>,
    /// Default batch size for bulk operations.
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
}

/// Common parameters for object storage operations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ObjectParams {
    /// Bucket name (S3 bucket, GCS bucket, Azure container).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bucket: Option<String>,
    /// Default prefix for object keys.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    /// Default batch size for bulk operations.
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
}

/// Common parameters for vector database operations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VectorParams {
    /// Collection or index name (Pinecone index, Qdrant collection).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collection: Option<String>,
    /// Dimension of vectors (required for some providers).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimension: Option<usize>,
    /// Distance metric for similarity search.
    #[serde(default)]
    pub metric: DistanceMetric,
    /// Default batch size for bulk operations.
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
}

/// Distance metric for vector similarity search.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DistanceMetric {
    /// Cosine similarity (default).
    #[default]
    Cosine,
    /// Euclidean distance (L2).
    Euclidean,
    /// Dot product.
    DotProduct,
}

fn default_batch_size() -> usize {
    1000
}
