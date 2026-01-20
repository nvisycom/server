//! PostgreSQL pgvector backend implementation.

use std::collections::HashMap;

use async_trait::async_trait;
use diesel::prelude::*;
use diesel::sql_types::{Float, Integer, Text};
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::pooled_connection::deadpool::Pool;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use nvisy_data::{
    DataError, DataResult, VectorContext, VectorData, VectorOutput, VectorSearchOptions,
    VectorSearchResult,
};

use super::{PgVectorConfig, PgVectorDistanceMetric, PgVectorIndexType};
use crate::TRACING_TARGET;

/// pgvector backend implementation using Diesel.
pub struct PgVectorBackend {
    pool: Pool<AsyncPgConnection>,
    config: PgVectorConfig,
}

impl PgVectorBackend {
    /// Creates a new pgvector backend.
    pub async fn new(config: &PgVectorConfig) -> DataResult<Self> {
        let manager =
            AsyncDieselConnectionManager::<AsyncPgConnection>::new(&config.connection_url);

        let pool = Pool::builder(manager)
            .build()
            .map_err(|e| DataError::connection(e.to_string()))?;

        // Test connection and ensure pgvector extension exists
        {
            let mut conn = pool
                .get()
                .await
                .map_err(|e| DataError::connection(e.to_string()))?;

            diesel::sql_query("CREATE EXTENSION IF NOT EXISTS vector")
                .execute(&mut conn)
                .await
                .map_err(|e| {
                    DataError::backend(format!("Failed to create vector extension: {}", e))
                })?;
        }

        tracing::debug!(
            target: TRACING_TARGET,
            table = %config.table,
            dimensions = %config.dimensions,
            "Initialized pgvector backend"
        );

        Ok(Self {
            pool,
            config: config.clone(),
        })
    }

    async fn get_conn(
        &self,
    ) -> DataResult<deadpool::managed::Object<AsyncDieselConnectionManager<AsyncPgConnection>>>
    {
        self.pool
            .get()
            .await
            .map_err(|e| DataError::connection(e.to_string()))
    }

    fn distance_operator(&self) -> &'static str {
        self.config.distance_metric.operator()
    }

    /// Ensures a collection (table) exists, creating it if necessary.
    async fn ensure_collection(&self, name: &str, dimensions: usize) -> DataResult<()> {
        let mut conn = self.get_conn().await?;

        // Create the table
        let create_table = format!(
            r#"
            CREATE TABLE IF NOT EXISTS {} (
                id VARCHAR(256) PRIMARY KEY,
                vector vector({}),
                metadata JSONB DEFAULT '{{}}'::jsonb,
                created_at TIMESTAMPTZ DEFAULT NOW()
            )
            "#,
            name, dimensions
        );

        diesel::sql_query(&create_table)
            .execute(&mut conn)
            .await
            .map_err(|e| DataError::backend(e.to_string()))?;

        // Create the index
        let index_name = format!("{}_vector_idx", name);
        let operator = self.distance_operator();

        let create_index = match self.config.index_type {
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
        };

        diesel::sql_query(&create_index)
            .execute(&mut conn)
            .await
            .map_err(|e| DataError::backend(e.to_string()))?;

        tracing::debug!(
            target: TRACING_TARGET,
            collection = %name,
            dimensions = %dimensions,
            "Ensured pgvector table exists"
        );

        Ok(())
    }
}

#[async_trait]
impl VectorOutput for PgVectorBackend {
    async fn insert(&self, ctx: &VectorContext, vectors: Vec<VectorData>) -> DataResult<()> {
        if vectors.is_empty() {
            return Ok(());
        }

        // Get dimensions from the first vector
        let dimensions = <[_]>::first(&vectors)
            .map(|v| v.vector.len())
            .ok_or_else(|| DataError::invalid("No vectors provided"))?;

        // Ensure collection exists
        self.ensure_collection(&ctx.collection, dimensions).await?;

        let mut conn = self.get_conn().await?;

        for v in vectors {
            let vector_str = format!(
                "[{}]",
                v.vector
                    .iter()
                    .map(|f| f.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            );
            let metadata_json =
                serde_json::to_string(&v.metadata).unwrap_or_else(|_| "{}".to_string());

            let upsert_query = format!(
                r#"
                INSERT INTO {} (id, vector, metadata)
                VALUES ($1, $2::vector, $3::jsonb)
                ON CONFLICT (id) DO UPDATE SET
                    vector = EXCLUDED.vector,
                    metadata = EXCLUDED.metadata
                "#,
                ctx.collection
            );

            diesel::sql_query(&upsert_query)
                .bind::<Text, _>(&v.id)
                .bind::<Text, _>(&vector_str)
                .bind::<Text, _>(&metadata_json)
                .execute(&mut conn)
                .await
                .map_err(|e| DataError::backend(e.to_string()))?;
        }

        Ok(())
    }

    async fn search(
        &self,
        ctx: &VectorContext,
        query: Vec<f32>,
        limit: usize,
        options: VectorSearchOptions,
    ) -> DataResult<Vec<VectorSearchResult>> {
        let mut conn = self.get_conn().await?;

        let operator = self.distance_operator();
        let vector_str = format!(
            "[{}]",
            query
                .iter()
                .map(|f| f.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );

        let vector_column = if options.include_vectors {
            ", vector::text as vector_data"
        } else {
            ""
        };

        // For cosine and inner product, we need to convert distance to similarity
        let score_expr = match self.config.distance_metric {
            PgVectorDistanceMetric::L2 => format!("vector {} $1::vector", operator),
            PgVectorDistanceMetric::InnerProduct => format!("-(vector {} $1::vector)", operator),
            PgVectorDistanceMetric::Cosine => format!("1 - (vector {} $1::vector)", operator),
        };

        let search_query = format!(
            r#"
            SELECT id, {} as score{}, metadata::text as metadata_json
            FROM {}
            ORDER BY vector {} $1::vector
            LIMIT $2
            "#,
            score_expr, vector_column, ctx.collection, operator
        );

        let results: Vec<SearchRow> = diesel::sql_query(&search_query)
            .bind::<Text, _>(&vector_str)
            .bind::<Integer, _>(limit as i32)
            .load(&mut conn)
            .await
            .map_err(|e| DataError::backend(e.to_string()))?;

        let search_results = results
            .into_iter()
            .map(|row| {
                let metadata: HashMap<String, serde_json::Value> =
                    serde_json::from_str(&row.metadata_json).unwrap_or_default();

                let vector = if options.include_vectors {
                    row.vector_data.and_then(|v| parse_vector(&v).ok())
                } else {
                    None
                };

                VectorSearchResult {
                    id: row.id,
                    score: row.score,
                    vector,
                    metadata,
                }
            })
            .collect();

        Ok(search_results)
    }
}

/// Parse a vector string from PostgreSQL format.
fn parse_vector(s: &str) -> DataResult<Vec<f32>> {
    let trimmed = s.trim_start_matches('[').trim_end_matches(']');
    trimmed
        .split(',')
        .map(|s| {
            s.trim()
                .parse::<f32>()
                .map_err(|e| DataError::serialization(e.to_string()))
        })
        .collect()
}

#[derive(QueryableByName)]
struct SearchRow {
    #[diesel(sql_type = Text)]
    id: String,
    #[diesel(sql_type = Float)]
    score: f32,
    #[diesel(sql_type = Text)]
    metadata_json: String,
    #[diesel(sql_type = diesel::sql_types::Nullable<Text>)]
    vector_data: Option<String>,
}
