//! pgvector configuration types.

use serde::{Deserialize, Serialize};

/// pgvector credentials (sensitive).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PgVectorCredentials {
    /// PostgreSQL connection URL.
    pub connection_url: String,
}

/// pgvector parameters (non-sensitive).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PgVectorParams {
    /// Table name for vectors.
    pub table: String,
    /// Vector dimensions.
    pub dimensions: usize,
    /// Distance metric.
    #[serde(default)]
    pub distance_metric: DistanceMetric,
    /// Index type for similarity search.
    #[serde(default)]
    pub index_type: IndexType,
}

/// Distance metric for pgvector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DistanceMetric {
    /// L2 (Euclidean) distance.
    #[default]
    L2,
    /// Inner product (dot product).
    InnerProduct,
    /// Cosine distance.
    Cosine,
}

impl DistanceMetric {
    /// Returns the pgvector operator for this metric.
    pub fn operator(&self) -> &'static str {
        match self {
            Self::L2 => "<->",
            Self::InnerProduct => "<#>",
            Self::Cosine => "<=>",
        }
    }
}

/// Index type for pgvector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexType {
    /// IVFFlat index (faster build, good recall).
    #[default]
    IvfFlat,
    /// HNSW index (slower build, better recall).
    Hnsw,
}
