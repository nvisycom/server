//! Pinecone vector store provider.

mod config;
mod output;

use std::collections::{BTreeMap, HashMap};

pub use config::{PineconeCredentials, PineconeParams};
use pinecone_sdk::models::{Kind, Metadata, Namespace, Value as PineconeValue};
use pinecone_sdk::pinecone::PineconeClientConfig;
use pinecone_sdk::pinecone::data::Index;
use tokio::sync::Mutex;

use crate::core::IntoProvider;
use crate::error::{Error, Result};

/// Pinecone provider for vector storage.
pub struct PineconeProvider {
    index: Mutex<Index>,
    params: PineconeParams,
}

#[async_trait::async_trait]
impl IntoProvider for PineconeProvider {
    type Credentials = PineconeCredentials;
    type Params = PineconeParams;

    async fn create(
        params: Self::Params,
        credentials: Self::Credentials,
    ) -> nvisy_core::Result<Self> {
        let client_config = PineconeClientConfig {
            api_key: Some(credentials.api_key),
            ..Default::default()
        };

        let client = client_config
            .client()
            .map_err(|e| Error::connection(e.to_string()))?;

        let index_description = client
            .describe_index(&params.index)
            .await
            .map_err(|e| Error::connection(format!("Failed to describe index: {}", e)))?;

        let host = &index_description.host;

        let index = client
            .index(host)
            .await
            .map_err(|e| Error::connection(format!("Failed to connect to index: {}", e)))?;

        Ok(Self {
            index: Mutex::new(index),
            params,
        })
    }
}

impl PineconeProvider {
    pub(crate) fn get_namespace(&self, collection: &str) -> Namespace {
        if collection.is_empty() {
            self.params
                .namespace
                .as_ref()
                .map(|ns| Namespace::from(ns.as_str()))
                .unwrap_or_default()
        } else {
            Namespace::from(collection)
        }
    }

    /// Returns the configured namespace.
    pub fn namespace(&self) -> Option<&str> {
        self.params.namespace.as_deref()
    }

    /// Searches for similar vectors.
    pub async fn search(
        &self,
        collection: &str,
        query: Vec<f32>,
        limit: usize,
        include_vectors: bool,
        include_metadata: bool,
        filter: Option<&serde_json::Value>,
    ) -> Result<Vec<SearchResult>> {
        let namespace = self.get_namespace(collection);

        let filter_metadata: Option<Metadata> = filter.and_then(|f| {
            if let serde_json::Value::Object(obj) = f {
                let map: HashMap<String, serde_json::Value> = obj.clone().into_iter().collect();
                Some(hashmap_to_metadata(map))
            } else {
                None
            }
        });

        let mut index = self.index.lock().await;
        let response = index
            .query_by_value(
                query,
                None,
                limit as u32,
                &namespace,
                filter_metadata,
                Some(include_vectors),
                Some(include_metadata),
            )
            .await
            .map_err(|e| Error::provider(e.to_string()))?;

        let results = response
            .matches
            .into_iter()
            .map(|m| {
                let metadata = m.metadata.map(metadata_to_hashmap).unwrap_or_default();

                SearchResult {
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

impl std::fmt::Debug for PineconeProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PineconeProvider").finish()
    }
}

fn metadata_to_hashmap(metadata: Metadata) -> HashMap<String, serde_json::Value> {
    metadata
        .fields
        .into_iter()
        .map(|(k, v)| (k, pinecone_value_to_json(v)))
        .collect()
}

pub(crate) fn hashmap_to_metadata(map: HashMap<String, serde_json::Value>) -> Metadata {
    let fields: BTreeMap<String, PineconeValue> = map
        .into_iter()
        .map(|(k, v)| (k, json_to_pinecone_value(v)))
        .collect();

    Metadata { fields }
}

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
