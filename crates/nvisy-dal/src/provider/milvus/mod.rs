//! Milvus vector store provider.

mod config;

use std::borrow::Cow;
use std::collections::HashMap;

use async_trait::async_trait;
pub use config::MilvusConfig;
use milvus::client::Client;
use milvus::collection::SearchOption;
use milvus::data::FieldColumn;
use milvus::index::{IndexParams, IndexType, MetricType};
use milvus::schema::{CollectionSchemaBuilder, FieldSchema};
use milvus::value::{Value, ValueVec};

use crate::core::{Context, DataInput, DataOutput, InputStream};
use crate::datatype::Embedding;
use crate::error::{Error, Result};

/// Milvus provider for vector storage.
pub struct MilvusProvider {
    client: Client,
    #[allow(dead_code)]
    config: MilvusConfig,
}

impl MilvusProvider {
    /// Creates a new Milvus provider.
    pub async fn new(config: &MilvusConfig) -> Result<Self> {
        let url = format!("http://{}:{}", config.host, config.port);

        let client = Client::new(url)
            .await
            .map_err(|e| Error::connection(e.to_string()))?;

        Ok(Self {
            client,
            config: config.clone(),
        })
    }

    /// Ensures a collection exists, creating it if necessary.
    async fn ensure_collection(&self, name: &str, dimensions: usize) -> Result<()> {
        let exists = self
            .client
            .has_collection(name)
            .await
            .map_err(|e| Error::provider(e.to_string()))?;

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
            .map_err(|e| Error::provider(e.to_string()))?;

        // Create the collection
        self.client
            .create_collection(schema, None)
            .await
            .map_err(|e| Error::provider(e.to_string()))?;

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
            .map_err(|e| Error::provider(e.to_string()))?;

        collection
            .create_index("vector", index_params)
            .await
            .map_err(|e| Error::provider(e.to_string()))?;

        // Load collection into memory
        collection
            .load(1)
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
    ) -> Result<Vec<SearchResult>> {
        let coll = self
            .client
            .get_collection(collection)
            .await
            .map_err(|e| Error::provider(e.to_string()))?;

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
            .map_err(|e| Error::provider(e.to_string()))?;

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

                search_results.push(SearchResult {
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
impl DataOutput<Embedding> for MilvusProvider {
    async fn write(&self, ctx: &Context, items: Vec<Embedding>) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }

        let collection = ctx
            .target
            .as_deref()
            .ok_or_else(|| Error::invalid_input("Collection name required in context.target"))?;

        // Get the dimension from the first vector
        let dim = items.first().map(|v| v.vector.len()).unwrap_or(0);

        // Ensure collection exists
        self.ensure_collection(collection, dim).await?;

        let coll = self
            .client
            .get_collection(collection)
            .await
            .map_err(|e| Error::provider(e.to_string()))?;

        let ids: Vec<String> = items.iter().map(|v| v.id.clone()).collect();
        let embeddings: Vec<f32> = items
            .iter()
            .flat_map(|v| v.vector.iter().copied())
            .collect();
        let metadata: Vec<String> = items
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
            .map_err(|e| Error::provider(e.to_string()))?;

        Ok(())
    }
}

#[async_trait]
impl DataInput<Embedding> for MilvusProvider {
    async fn read(&self, _ctx: &Context) -> Result<InputStream<'static, Embedding>> {
        // Vector stores are primarily write/search, not sequential read
        let stream = futures::stream::empty();
        Ok(InputStream::new(Box::pin(stream)))
    }
}

impl std::fmt::Debug for MilvusProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MilvusProvider").finish()
    }
}
