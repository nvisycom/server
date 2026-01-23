//! PostgreSQL pgvector provider.

mod config;
mod output;

use std::collections::HashMap;

pub use config::{DistanceMetric, IndexType, PgVectorCredentials, PgVectorParams};
use diesel::prelude::*;
use diesel::sql_types::{Float, Integer, Text};
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::pooled_connection::deadpool::Pool;
use diesel_async::{AsyncPgConnection, RunQueryDsl};

use crate::core::IntoProvider;
use crate::error::{Error, Result};

/// pgvector provider for vector storage using PostgreSQL.
pub struct PgVectorProvider {
    pool: Pool<AsyncPgConnection>,
    params: PgVectorParams,
}

#[async_trait::async_trait]
impl IntoProvider for PgVectorProvider {
    type Credentials = PgVectorCredentials;
    type Params = PgVectorParams;

    async fn create(
        params: Self::Params,
        credentials: Self::Credentials,
    ) -> nvisy_core::Result<Self> {
        let manager =
            AsyncDieselConnectionManager::<AsyncPgConnection>::new(&credentials.connection_url);

        let pool = Pool::builder(manager)
            .build()
            .map_err(|e| Error::connection(e.to_string()))?;

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

        Ok(Self { pool, params })
    }
}

impl PgVectorProvider {
    /// Returns the configured table name.
    pub fn table(&self) -> &str {
        &self.params.table
    }

    pub(crate) async fn get_conn(
        &self,
    ) -> Result<deadpool::managed::Object<AsyncDieselConnectionManager<AsyncPgConnection>>> {
        self.pool
            .get()
            .await
            .map_err(|e| Error::connection(e.to_string()))
    }

    pub(crate) fn distance_operator(&self) -> &'static str {
        self.params.distance_metric.operator()
    }

    /// Ensures a collection (table) exists, creating it if necessary.
    pub(crate) async fn ensure_collection(&self, name: &str, dimensions: usize) -> Result<()> {
        let mut conn = self.get_conn().await?;

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

        let index_name = format!("{}_vector_idx", name);
        let operator = self.distance_operator();

        let create_index = match self.params.index_type {
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

        let score_expr = match self.params.distance_metric {
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

impl std::fmt::Debug for PgVectorProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PgVectorProvider").finish()
    }
}

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
