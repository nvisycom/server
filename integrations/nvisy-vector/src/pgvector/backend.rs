//! pgvector backend implementation.

use std::collections::HashMap;

use async_trait::async_trait;

use super::{PgVectorConfig, PgVectorDistanceMetric, PgVectorIndexType};
use crate::TRACING_TARGET;
use crate::error::{VectorError, VectorResult};
use crate::store::{SearchOptions, SearchResult, VectorData, VectorStoreBackend};

/// pgvector backend implementation.
///
/// This backend uses raw SQL queries via the pgvector extension.
/// It's designed to work with any PostgreSQL async driver.
pub struct PgVectorBackend {
    config: PgVectorConfig,
    // In a real implementation, this would hold a connection pool
    // For now, we store the connection URL for documentation purposes
    #[allow(dead_code)]
    connection_url: String,
}

impl PgVectorBackend {
    /// Creates a new pgvector backend.
    pub async fn new(config: &PgVectorConfig) -> VectorResult<Self> {
        // In a real implementation, we would:
        // 1. Create a connection pool
        // 2. Verify pgvector extension is installed
        // 3. Test the connection

        tracing::debug!(
            target: TRACING_TARGET,
            table = %config.table,
            dimensions = %config.dimensions,
            "Initialized pgvector backend"
        );

        Ok(Self {
            config: config.clone(),
            connection_url: config.connection_url.clone(),
        })
    }

    /// Generates SQL for creating the vectors table.
    pub fn create_table_sql(&self, name: &str, dimensions: usize) -> String {
        format!(
            r#"
            CREATE TABLE IF NOT EXISTS {} (
                id VARCHAR(256) PRIMARY KEY,
                vector vector({}),
                metadata JSONB DEFAULT '{{}}'::jsonb,
                created_at TIMESTAMPTZ DEFAULT NOW()
            )
            "#,
            name, dimensions
        )
    }

    /// Generates SQL for creating the vector index.
    pub fn create_index_sql(&self, name: &str) -> String {
        let index_name = format!("{}_vector_idx", name);
        let operator = self.config.distance_metric.operator();

        match self.config.index_type {
            PgVectorIndexType::IvfFlat => {
                format!(
                    r#"
                    CREATE INDEX IF NOT EXISTS {} ON {}
                    USING ivfflat (vector {})
                    WITH (lists = 100)
                    "#,
                    index_name, name, operator
                )
            }
            PgVectorIndexType::Hnsw => {
                format!(
                    r#"
                    CREATE INDEX IF NOT EXISTS {} ON {}
                    USING hnsw (vector {})
                    WITH (m = 16, ef_construction = 64)
                    "#,
                    index_name, name, operator
                )
            }
        }
    }

    /// Generates SQL for upserting vectors.
    pub fn upsert_sql(&self, name: &str) -> String {
        format!(
            r#"
            INSERT INTO {} (id, vector, metadata)
            VALUES ($1, $2, $3)
            ON CONFLICT (id) DO UPDATE SET
                vector = EXCLUDED.vector,
                metadata = EXCLUDED.metadata
            "#,
            name
        )
    }

    /// Generates SQL for searching vectors.
    pub fn search_sql(&self, name: &str, include_vector: bool) -> String {
        let operator = self.config.distance_metric.operator();
        let vector_column = if include_vector { ", vector" } else { "" };

        let distance_expr = match self.config.distance_metric {
            PgVectorDistanceMetric::L2 => format!("vector {} $1", operator),
            PgVectorDistanceMetric::InnerProduct => {
                // Inner product returns negative, so we negate for similarity
                format!("-(vector {} $1)", operator)
            }
            PgVectorDistanceMetric::Cosine => {
                // Cosine distance, convert to similarity
                format!("1 - (vector {} $1)", operator)
            }
        };

        format!(
            r#"
            SELECT id, {} as score{}, metadata
            FROM {}
            ORDER BY vector {} $1
            LIMIT $2
            "#,
            distance_expr, vector_column, name, operator
        )
    }

    /// Generates SQL for deleting vectors.
    pub fn delete_sql(&self, name: &str) -> String {
        format!("DELETE FROM {} WHERE id = ANY($1)", name)
    }

    /// Generates SQL for getting vectors by ID.
    pub fn get_sql(&self, name: &str) -> String {
        format!(
            "SELECT id, vector, metadata FROM {} WHERE id = ANY($1)",
            name
        )
    }
}

#[async_trait]
impl VectorStoreBackend for PgVectorBackend {
    async fn create_collection(&self, name: &str, dimensions: usize) -> VectorResult<()> {
        // In a real implementation, execute:
        // 1. CREATE EXTENSION IF NOT EXISTS vector;
        // 2. self.create_table_sql(name, dimensions)
        // 3. self.create_index_sql(name)

        tracing::info!(
            target: TRACING_TARGET,
            collection = %name,
            dimensions = %dimensions,
            index_type = ?self.config.index_type,
            "Would create pgvector table: {}",
            self.create_table_sql(name, dimensions)
        );

        Ok(())
    }

    async fn delete_collection(&self, name: &str) -> VectorResult<()> {
        // In a real implementation, execute:
        // DROP TABLE IF EXISTS {name}

        tracing::info!(
            target: TRACING_TARGET,
            collection = %name,
            "Would drop pgvector table"
        );

        Ok(())
    }

    async fn collection_exists(&self, _name: &str) -> VectorResult<bool> {
        // In a real implementation, query information_schema.tables
        Ok(true)
    }

    async fn upsert(&self, collection: &str, vectors: Vec<VectorData>) -> VectorResult<()> {
        // In a real implementation, execute batched upserts
        let sql = self.upsert_sql(collection);

        tracing::debug!(
            target: TRACING_TARGET,
            collection = %collection,
            count = %vectors.len(),
            "Would upsert with SQL: {}",
            sql
        );

        Ok(())
    }

    async fn search(
        &self,
        collection: &str,
        _query: Vec<f32>,
        _limit: usize,
        options: SearchOptions,
    ) -> VectorResult<Vec<SearchResult>> {
        let sql = self.search_sql(collection, options.include_vectors);

        tracing::debug!(
            target: TRACING_TARGET,
            collection = %collection,
            "Would search with SQL: {}",
            sql
        );

        // In a real implementation, execute the query and parse results
        Ok(vec![])
    }

    async fn delete(&self, collection: &str, ids: Vec<String>) -> VectorResult<()> {
        let sql = self.delete_sql(collection);

        tracing::debug!(
            target: TRACING_TARGET,
            collection = %collection,
            count = %ids.len(),
            "Would delete with SQL: {}",
            sql
        );

        Ok(())
    }

    async fn get(&self, collection: &str, ids: Vec<String>) -> VectorResult<Vec<VectorData>> {
        let sql = self.get_sql(collection);

        tracing::debug!(
            target: TRACING_TARGET,
            collection = %collection,
            count = %ids.len(),
            "Would get with SQL: {}",
            sql
        );

        // In a real implementation, execute the query and parse results
        Ok(vec![])
    }
}

/// Helper to format a vector for PostgreSQL.
#[allow(dead_code)]
pub fn format_vector(v: &[f32]) -> String {
    format!(
        "[{}]",
        v.iter()
            .map(|f| f.to_string())
            .collect::<Vec<_>>()
            .join(",")
    )
}

/// Helper to parse a vector from PostgreSQL.
#[allow(dead_code)]
pub fn parse_vector(s: &str) -> VectorResult<Vec<f32>> {
    let trimmed = s.trim_start_matches('[').trim_end_matches(']');
    trimmed
        .split(',')
        .map(|s| {
            s.trim()
                .parse::<f32>()
                .map_err(|e| VectorError::serialization(e.to_string()))
        })
        .collect()
}

/// Helper to convert metadata to JSONB.
#[allow(dead_code)]
pub fn metadata_to_jsonb(metadata: &HashMap<String, serde_json::Value>) -> String {
    serde_json::to_string(metadata).unwrap_or_else(|_| "{}".to_string())
}
