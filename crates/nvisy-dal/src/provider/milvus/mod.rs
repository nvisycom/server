//! Milvus vector store provider.

mod config;
mod output;

use std::borrow::Cow;
use std::collections::HashMap;

pub use config::{MilvusCredentials, MilvusParams};
use milvus::client::Client;
use milvus::collection::SearchOption;
use milvus::index::{IndexParams, IndexType, MetricType};
use milvus::schema::{CollectionSchemaBuilder, FieldSchema};
use milvus::value::Value;

use crate::core::IntoProvider;
use crate::error::{Error, Result};

/// Milvus provider for vector storage.
pub struct MilvusProvider {
    client: Client,
    params: MilvusParams,
}

#[async_trait::async_trait]
impl IntoProvider for MilvusProvider {
    type Credentials = MilvusCredentials;
    type Params = MilvusParams;

    async fn create(
        params: Self::Params,
        credentials: Self::Credentials,
    ) -> nvisy_core::Result<Self> {
        let url = format!("http://{}:{}", credentials.host, credentials.port);

        let client = Client::new(url)
            .await
            .map_err(|e| Error::connection(e.to_string()))?;

        Ok(Self { client, params })
    }
}

impl MilvusProvider {
    /// Returns the configured collection name.
    pub fn collection(&self) -> &str {
        &self.params.collection
    }

    /// Ensures a collection exists, creating it if necessary.
    pub(crate) async fn ensure_collection(&self, name: &str, dimensions: usize) -> Result<()> {
        let exists = self
            .client
            .has_collection(name)
            .await
            .map_err(|e| Error::provider(e.to_string()))?;

        if exists {
            return Ok(());
        }

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

        self.client
            .create_collection(schema, None)
            .await
            .map_err(|e| Error::provider(e.to_string()))?;

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

impl std::fmt::Debug for MilvusProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MilvusProvider").finish()
    }
}
