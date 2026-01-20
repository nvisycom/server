//! PostgreSQL pgvector provider.

mod config;

use std::collections::HashMap;

use async_trait::async_trait;
pub use config::{DistanceMetric, IndexType, PgVectorConfig};
use diesel::prelude::*;
use diesel::sql_types::{Float, Integer, Text};
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::pooled_connection::deadpool::Pool;
use diesel_async::{AsyncPgConnection, RunQueryDsl};

use crate::core::{Context, DataInput, DataOutput, InputStream};
use crate::datatype::Embedding;
use crate::error::{Error, Result};

/// pgvector provider for vector storage using PostgreSQL.
pub struct PgVectorProvider {
    pool: Pool<AsyncPgConnection>,
    config: PgVectorConfig,
}

impl PgVectorProvider {
    /// Creates a new pgvector provider.
    pub async fn new(config: &PgVectorConfig) -> Result<Self> {
        let manager =
            AsyncDieselConnectionManager::<AsyncPgConnection>::new(&config.connection_url);

        let pool = Pool::builder(manager)
            .build()
            .map_err(|e| Error::connection(e.to_string()))?;

        // Test connection and ensure pgvector extension exists
        {
            let mut conn = pool
                .get()
                .await
                .map_err(|e| Error::connection(e.to_string()))?;

            diesel::sql_query("CREATE EXTENSION IF NOT EXISTS vector")
                .execute(&mut conn)
                .await
                .map_err(|e| {
                    Error::provider(format!("Failed to create vector extension: {}", e))
                })?;
        }

        Ok(Self {
            pool,
            config: config.clone(),
        })
    }

    async fn get_conn(
        &self,
    ) -> Result<deadpool::managed::Object<AsyncDieselConnectionManager<AsyncPgConnection>>> {
        self.pool
            .get()
            .await
            .map_err(|e| Error::connection(e.to_string()))
    }

    fn distance_operator(&self) -> &'static str {
        self.config.distance_metric.operator()
    }

    /// Ensures a collection (table) exists, creating it if necessary.
    async fn ensure_collection(&self, name: &str, dimensions: usize) -> Result<()> {
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
            .map_err(|e| Error::provider(e.to_string()))?;

        // Create the index
        let index_name = format!("{}_vector_idx", name);
        let operator = self.distance_operator();

        let create_index = match self.config.index_type {
            IndexType::IvfFlat => {
                format!(
                    r#"
                    CREATE INDEX IF NOT EXISTS {} ON {}
                    USING ivfflat (vector {})
                    WITH (lists = 100)
                    "#,
                    index_name, name, operator
                )
            }
            IndexType::Hnsw => {
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
            .map_err(|e| Error::provider(e.to_string()))?;

        Ok(())
    }

    /// Searches for similar vectors.
    pub async fn search(
        &self,
        collection: &str,
        query: Vec<f32>,
        limit: usize,
        include_vectors: bool,
    ) -> Result<Vec<SearchResult>> {
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

        let vector_column = if include_vectors {
            ", vector::text as vector_data"
        } else {
            ""
        };

        // For cosine and inner product, convert distance to similarity
        let score_expr = match self.config.distance_metric {
            DistanceMetric::L2 => format!("vector {} $1::vector", operator),
            DistanceMetric::InnerProduct => format!("-(vector {} $1::vector)", operator),
            DistanceMetric::Cosine => format!("1 - (vector {} $1::vector)", operator),
        };

        let search_query = format!(
            r#"
            SELECT id, {} as score{}, metadata::text as metadata_json
            FROM {}
            ORDER BY vector {} $1::vector
            LIMIT $2
            "#,
            score_expr, vector_column, collection, operator
        );

        let results: Vec<SearchRow> = diesel::sql_query(&search_query)
            .bind::<Text, _>(&vector_str)
            .bind::<Integer, _>(limit as i32)
            .load(&mut conn)
            .await
            .map_err(|e| Error::provider(e.to_string()))?;

        let search_results = results
            .into_iter()
            .map(|row| {
                let metadata: HashMap<String, serde_json::Value> =
                    serde_json::from_str(&row.metadata_json).unwrap_or_default();

                let vector = if include_vectors {
                    row.vector_data.and_then(|v| parse_vector(&v).ok())
                } else {
                    None
                };

                SearchResult {
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

/// Result from a vector similarity search.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// The ID of the matched vector.
    pub id: String,
    /// Similarity score.
    pub score: f32,
    /// The vector data, if requested.
    pub vector: Option<Vec<f32>>,
    /// Metadata associated with this vector.
    pub metadata: HashMap<String, serde_json::Value>,
}

#[async_trait]
impl DataOutput<Embedding> for PgVectorProvider {
    async fn write(&self, ctx: &Context, items: Vec<Embedding>) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }

        let collection = ctx
            .target
            .as_deref()
            .ok_or_else(|| Error::invalid_input("Collection name required in context.target"))?;

        // Get dimensions from the first vector
        let dimensions = <[_]>::first(&items)
            .map(|v| v.vector.len())
            .ok_or_else(|| Error::invalid_input("No embeddings provided"))?;

        // Ensure collection exists
        self.ensure_collection(collection, dimensions).await?;

        let mut conn = self.get_conn().await?;

        for v in items {
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
                collection
            );

            diesel::sql_query(&upsert_query)
                .bind::<Text, _>(&v.id)
                .bind::<Text, _>(&vector_str)
                .bind::<Text, _>(&metadata_json)
                .execute(&mut conn)
                .await
                .map_err(|e| Error::provider(e.to_string()))?;
        }

        Ok(())
    }
}

#[async_trait]
impl DataInput<Embedding> for PgVectorProvider {
    async fn read(&self, _ctx: &Context) -> Result<InputStream<'static, Embedding>> {
        // Vector stores are primarily write/search, not sequential read
        let stream = futures::stream::empty();
        Ok(InputStream::new(Box::pin(stream)))
    }
}

impl std::fmt::Debug for PgVectorProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PgVectorProvider").finish()
    }
}

/// Parse a vector string from PostgreSQL format.
fn parse_vector(s: &str) -> Result<Vec<f32>> {
    let trimmed = s.trim_start_matches('[').trim_end_matches(']');
    trimmed
        .split(',')
        .map(|s| {
            s.trim()
                .parse::<f32>()
                .map_err(|e| Error::provider(e.to_string()))
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
