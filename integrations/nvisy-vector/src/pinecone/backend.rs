//! Pinecone backend implementation.

use std::collections::{BTreeMap, HashMap};

use async_trait::async_trait;
use nvisy_data::{
    DataError, DataResult, VectorContext, VectorData, VectorOutput, VectorSearchOptions,
    VectorSearchResult,
};
use pinecone_sdk::models::{Kind, Metadata, Namespace, Value as PineconeValue, Vector};
use pinecone_sdk::pinecone::PineconeClientConfig;
use pinecone_sdk::pinecone::data::Index;
use tokio::sync::Mutex;

use super::PineconeConfig;
use crate::TRACING_TARGET;

/// Pinecone backend implementation.
pub struct PineconeBackend {
    index: Mutex<Index>,
    config: PineconeConfig,
}

impl PineconeBackend {
    /// Creates a new Pinecone backend.
    pub async fn new(config: &PineconeConfig) -> DataResult<Self> {
        let client_config = PineconeClientConfig {
            api_key: Some(config.api_key.clone()),
            ..Default::default()
        };

        let client = client_config
            .client()
            .map_err(|e| DataError::connection(e.to_string()))?;

        // Describe the index to get its host
        let index_description = client
            .describe_index(&config.index)
            .await
            .map_err(|e| DataError::connection(format!("Failed to describe index: {}", e)))?;

        // host is a String, not Option<String>
        let host = &index_description.host;

        // Connect to the index
        let index = client
            .index(host)
            .await
            .map_err(|e| DataError::connection(format!("Failed to connect to index: {}", e)))?;

        tracing::debug!(
            target: TRACING_TARGET,
            index = %config.index,
            "Connected to Pinecone"
        );

        Ok(Self {
            index: Mutex::new(index),
            config: config.clone(),
        })
    }

    fn get_namespace(&self, collection: &str) -> Namespace {
        if collection.is_empty() {
            self.config
                .namespace
                .as_ref()
                .map(|ns| Namespace::from(ns.as_str()))
                .unwrap_or_default()
        } else {
            Namespace::from(collection)
        }
    }

    /// Convert Pinecone Metadata (prost_types::Struct) to HashMap
    fn metadata_to_hashmap(metadata: Metadata) -> HashMap<String, serde_json::Value> {
        metadata
            .fields
            .into_iter()
            .map(|(k, v)| (k, pinecone_value_to_json(v)))
            .collect()
    }

    /// Convert HashMap to Pinecone Metadata (prost_types::Struct)
    fn hashmap_to_metadata(map: HashMap<String, serde_json::Value>) -> Metadata {
        let fields: BTreeMap<String, PineconeValue> = map
            .into_iter()
            .map(|(k, v)| (k, json_to_pinecone_value(v)))
            .collect();

        Metadata { fields }
    }
}

#[async_trait]
impl VectorOutput for PineconeBackend {
    async fn insert(&self, ctx: &VectorContext, vectors: Vec<VectorData>) -> DataResult<()> {
        let namespace = self.get_namespace(&ctx.collection);

        let pinecone_vectors: Vec<Vector> = vectors
            .into_iter()
            .map(|v| {
                let metadata = if v.metadata.is_empty() {
                    None
                } else {
                    Some(Self::hashmap_to_metadata(v.metadata))
                };

                Vector {
                    id: v.id,
                    values: v.vector,
                    sparse_values: None,
                    metadata,
                }
            })
            .collect();

        let mut index = self.index.lock().await;
        index
            .upsert(&pinecone_vectors, &namespace)
            .await
            .map_err(|e| DataError::backend(e.to_string()))?;

        Ok(())
    }

    async fn search(
        &self,
        ctx: &VectorContext,
        query: Vec<f32>,
        limit: usize,
        options: VectorSearchOptions,
    ) -> DataResult<Vec<VectorSearchResult>> {
        let namespace = self.get_namespace(&ctx.collection);

        let filter: Option<Metadata> = options.filter.and_then(|f| {
            if let serde_json::Value::Object(obj) = f {
                let map: HashMap<String, serde_json::Value> = obj.into_iter().collect();
                Some(Self::hashmap_to_metadata(map))
            } else {
                None
            }
        });

        let mut index = self.index.lock().await;
        let response = index
            .query_by_value(
                query,
                None, // sparse values
                limit as u32,
                &namespace,
                filter,
                Some(options.include_vectors),
                Some(options.include_metadata),
            )
            .await
            .map_err(|e| DataError::backend(e.to_string()))?;

        let results = response
            .matches
            .into_iter()
            .map(|m| {
                let metadata = m
                    .metadata
                    .map(Self::metadata_to_hashmap)
                    .unwrap_or_default();

                VectorSearchResult {
                    id: m.id,
                    score: m.score,
                    vector: Some(m.values),
                    metadata,
                }
            })
            .collect();

        Ok(results)
    }
}

/// Convert Pinecone Value (prost_types::Value) to serde_json::Value
fn pinecone_value_to_json(value: PineconeValue) -> serde_json::Value {
    match value.kind {
        Some(Kind::NullValue(_)) => serde_json::Value::Null,
        Some(Kind::NumberValue(n)) => serde_json::Value::Number(
            serde_json::Number::from_f64(n).unwrap_or(serde_json::Number::from(0)),
        ),
        Some(Kind::StringValue(s)) => serde_json::Value::String(s),
        Some(Kind::BoolValue(b)) => serde_json::Value::Bool(b),
        Some(Kind::StructValue(s)) => {
            let map: serde_json::Map<String, serde_json::Value> = s
                .fields
                .into_iter()
                .map(|(k, v)| (k, pinecone_value_to_json(v)))
                .collect();
            serde_json::Value::Object(map)
        }
        Some(Kind::ListValue(list)) => {
            let arr: Vec<serde_json::Value> = list
                .values
                .into_iter()
                .map(pinecone_value_to_json)
                .collect();
            serde_json::Value::Array(arr)
        }
        None => serde_json::Value::Null,
    }
}

/// Convert serde_json::Value to Pinecone Value (prost_types::Value)
fn json_to_pinecone_value(value: serde_json::Value) -> PineconeValue {
    let kind = match value {
        serde_json::Value::Null => Some(Kind::NullValue(0)),
        serde_json::Value::Bool(b) => Some(Kind::BoolValue(b)),
        serde_json::Value::Number(n) => Some(Kind::NumberValue(n.as_f64().unwrap_or(0.0))),
        serde_json::Value::String(s) => Some(Kind::StringValue(s)),
        serde_json::Value::Array(arr) => Some(Kind::ListValue(prost_types::ListValue {
            values: arr.into_iter().map(json_to_pinecone_value).collect(),
        })),
        serde_json::Value::Object(obj) => {
            let fields: BTreeMap<String, PineconeValue> = obj
                .into_iter()
                .map(|(k, v)| (k, json_to_pinecone_value(v)))
                .collect();
            Some(Kind::StructValue(prost_types::Struct { fields }))
        }
    };

    PineconeValue { kind }
}
