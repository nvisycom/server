//! Milvus backend implementation.

use std::borrow::Cow;
use std::collections::HashMap;

use async_trait::async_trait;
use milvus::client::Client;
use milvus::collection::SearchOption;
use milvus::data::FieldColumn;
use milvus::index::{IndexParams, IndexType, MetricType};
use milvus::schema::{CollectionSchemaBuilder, FieldSchema};
use milvus::value::{Value, ValueVec};
use nvisy_data::{
    DataError, DataResult, VectorContext, VectorData, VectorOutput, VectorSearchOptions,
    VectorSearchResult,
};

use super::MilvusConfig;
use crate::TRACING_TARGET;

/// Milvus backend implementation.
pub struct MilvusBackend {
    client: Client,
    #[allow(dead_code)]
    config: MilvusConfig,
}

impl MilvusBackend {
    /// Creates a new Milvus backend.
    pub async fn new(config: &MilvusConfig) -> DataResult<Self> {
        let url = format!("http://{}:{}", config.host, config.port);

        let client = Client::new(url)
            .await
            .map_err(|e| DataError::connection(e.to_string()))?;

        tracing::debug!(
            target: TRACING_TARGET,
            host = %config.host,
            port = %config.port,
            "Connected to Milvus"
        );

        Ok(Self {
            client,
            config: config.clone(),
        })
    }

    /// Ensures a collection exists, creating it if necessary.
    async fn ensure_collection(&self, name: &str, dimensions: usize) -> DataResult<()> {
        let exists = self
            .client
            .has_collection(name)
            .await
            .map_err(|e| DataError::backend(e.to_string()))?;

        if exists {
            return Ok(());
        }

        // Build the collection schema
        let mut builder = CollectionSchemaBuilder::new(name, "Vector collection");
        builder.add_field(FieldSchema::new_primary_int64("_id", "primary key", true));
        builder.add_field(FieldSchema::new_varchar("id", "string id", 256));
        builder.add_field(FieldSchema::new_float_vector(
            "vector",
            "embedding vector",
            dimensions as i64,
        ));
        builder.add_field(FieldSchema::new_varchar("metadata", "json metadata", 65535));

        let schema = builder
            .build()
            .map_err(|e| DataError::backend(e.to_string()))?;

        // Create the collection
        self.client
            .create_collection(schema, None)
            .await
            .map_err(|e| DataError::backend(e.to_string()))?;

        // Create index on vector field
        let index_params = IndexParams::new(
            "vector_index".to_string(),
            IndexType::IvfFlat,
            MetricType::L2,
            HashMap::from([("nlist".to_string(), "128".to_string())]),
        );

        let collection = self
            .client
            .get_collection(name)
            .await
            .map_err(|e| DataError::backend(e.to_string()))?;

        collection
            .create_index("vector", index_params)
            .await
            .map_err(|e| DataError::backend(e.to_string()))?;

        // Load collection into memory
        collection
            .load(1)
            .await
            .map_err(|e| DataError::backend(e.to_string()))?;

        tracing::info!(
            target: TRACING_TARGET,
            collection = %name,
            dimensions = %dimensions,
            "Created Milvus collection"
        );

        Ok(())
    }
}

#[async_trait]
impl VectorOutput for MilvusBackend {
    async fn insert(&self, ctx: &VectorContext, vectors: Vec<VectorData>) -> DataResult<()> {
        if vectors.is_empty() {
            return Ok(());
        }

        // Get the dimension from the first vector
        let dim = vectors.first().map(|v| v.vector.len()).unwrap_or(0);

        // Ensure collection exists
        self.ensure_collection(&ctx.collection, dim).await?;

        let coll = self
            .client
            .get_collection(&ctx.collection)
            .await
            .map_err(|e| DataError::backend(e.to_string()))?;

        let ids: Vec<String> = vectors.iter().map(|v| v.id.clone()).collect();
        let embeddings: Vec<f32> = vectors
            .iter()
            .flat_map(|v| v.vector.iter().copied())
            .collect();
        let metadata: Vec<String> = vectors
            .iter()
            .map(|v| serde_json::to_string(&v.metadata).unwrap_or_default())
            .collect();

        // Create field schemas for columns
        let id_schema = FieldSchema::new_varchar("id", "string id", 256);
        let vector_schema = FieldSchema::new_float_vector("vector", "embedding vector", dim as i64);
        let metadata_schema = FieldSchema::new_varchar("metadata", "json metadata", 65535);

        let columns = vec![
            FieldColumn::new(&id_schema, ValueVec::String(ids)),
            FieldColumn::new(&vector_schema, ValueVec::Float(embeddings)),
            FieldColumn::new(&metadata_schema, ValueVec::String(metadata)),
        ];

        coll.insert(columns, None)
            .await
            .map_err(|e| DataError::backend(e.to_string()))?;

        Ok(())
    }

    async fn search(
        &self,
        ctx: &VectorContext,
        query: Vec<f32>,
        limit: usize,
        _options: VectorSearchOptions,
    ) -> DataResult<Vec<VectorSearchResult>> {
        let coll = self
            .client
            .get_collection(&ctx.collection)
            .await
            .map_err(|e| DataError::backend(e.to_string()))?;

        let mut search_option = SearchOption::new();
        search_option.add_param("nprobe", serde_json::json!(16));

        let query_value = Value::FloatArray(Cow::Owned(query));

        let results = coll
            .search(
                vec![query_value],
                "vector",
                limit as i32,
                MetricType::L2,
                vec!["id", "metadata"],
                &search_option,
            )
            .await
            .map_err(|e| DataError::backend(e.to_string()))?;

        let mut search_results = Vec::new();

        for result in results {
            for i in 0..result.size as usize {
                let id = match result.id.get(i) {
                    Some(Value::String(s)) => s.to_string(),
                    Some(Value::Long(l)) => l.to_string(),
                    _ => continue,
                };

                let score = result.score.get(i).copied().unwrap_or(0.0);

                // Extract metadata from fields
                let metadata_str = result
                    .field
                    .iter()
                    .find(|f| f.name == "metadata")
                    .and_then(|f| f.get(i))
                    .and_then(|v| match v {
                        Value::String(s) => Some(s.to_string()),
                        _ => None,
                    });

                let metadata: HashMap<String, serde_json::Value> = metadata_str
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or_default();

                // Get string id if available
                let string_id = result
                    .field
                    .iter()
                    .find(|f| f.name == "id")
                    .and_then(|f| f.get(i))
                    .and_then(|v| match v {
                        Value::String(s) => Some(s.to_string()),
                        _ => None,
                    })
                    .unwrap_or(id);

                search_results.push(VectorSearchResult {
                    id: string_id,
                    score,
                    vector: None,
                    metadata,
                });
            }
        }

        Ok(search_results)
    }
}
