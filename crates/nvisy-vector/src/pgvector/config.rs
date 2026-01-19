//! pgvector configuration.

use serde::{Deserialize, Serialize};

/// PostgreSQL pgvector configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PgVectorConfig {
    /// PostgreSQL connection URL.
    pub connection_url: String,
    /// Table name for vectors.
    #[serde(default = "default_pgvector_table")]
    pub table: String,
    /// Vector dimensions.
    pub dimensions: usize,
    /// Distance metric.
    #[serde(default)]
    pub distance_metric: PgVectorDistanceMetric,
    /// Index type for similarity search.
    #[serde(default)]
    pub index_type: PgVectorIndexType,
}

impl PgVectorConfig {
    /// Creates a new pgvector configuration.
    pub fn new(connection_url: impl Into<String>, dimensions: usize) -> Self {
        Self {
            connection_url: connection_url.into(),
            table: default_pgvector_table(),
            dimensions,
            distance_metric: PgVectorDistanceMetric::default(),
            index_type: PgVectorIndexType::default(),
        }
    }

    /// Sets the table name.
    pub fn with_table(mut self, table: impl Into<String>) -> Self {
        self.table = table.into();
        self
    }

    /// Sets the distance metric.
    pub fn with_distance_metric(mut self, metric: PgVectorDistanceMetric) -> Self {
        self.distance_metric = metric;
        self
    }

    /// Sets the index type.
    pub fn with_index_type(mut self, index_type: PgVectorIndexType) -> Self {
        self.index_type = index_type;
        self
    }
}

fn default_pgvector_table() -> String {
    "vectors".to_string()
}

/// Distance metric for pgvector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PgVectorDistanceMetric {
    /// L2 (Euclidean) distance.
    #[default]
    L2,
    /// Inner product (dot product).
    InnerProduct,
    /// Cosine distance.
    Cosine,
}

impl PgVectorDistanceMetric {
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
pub enum PgVectorIndexType {
    /// IVFFlat index (faster build, good recall).
    #[default]
    IvfFlat,
    /// HNSW index (slower build, better recall).
    Hnsw,
}
