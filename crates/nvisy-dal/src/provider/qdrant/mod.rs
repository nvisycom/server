//! Qdrant vector store provider.

mod config;
mod output;

use std::collections::HashMap;

pub use config::QdrantConfig;
use qdrant_client::Qdrant;
use qdrant_client::qdrant::vectors_config::Config as VectorsConfig;
use qdrant_client::qdrant::with_payload_selector::SelectorOptions;
use qdrant_client::qdrant::with_vectors_selector::SelectorOptions as VectorsSelectorOptions;
use qdrant_client::qdrant::{
    Condition, CreateCollectionBuilder, Distance, Filter, PointId, SearchPointsBuilder,
    VectorParamsBuilder,
};

use crate::error::{Error, Result};

/// Qdrant provider for vector storage.
pub struct QdrantProvider {
    client: Qdrant,
    config: QdrantConfig,
}

impl QdrantProvider {
    /// Creates a new Qdrant provider.
    pub async fn new(config: &QdrantConfig) -> Result<Self> {
        let client = Qdrant::from_url(&config.url)
            .api_key(config.api_key.clone())
            .build()
            .map_err(|e| Error::connection(e.to_string()))?;

        Ok(Self {
            client,
            config: config.clone(),
        })
    }

    /// Ensures a collection exists, creating it if necessary.
    pub(crate) async fn ensure_collection(&self, name: &str, dimensions: usize) -> Result<()> {
        let exists = self
            .client
            .collection_exists(name)
            .await
            .map_err(|e| Error::provider(e.to_string()))?;

        if !exists {
            let vectors_config = VectorsConfig::Params(
                VectorParamsBuilder::new(dimensions as u64, Distance::Cosine).build(),
            );

            self.client
                .create_collection(
                    CreateCollectionBuilder::new(name).vectors_config(vectors_config),
                )
                .await
                .map_err(|e| Error::provider(e.to_string()))?;
        }

        Ok(())
    }

    /// Returns the configured collection name.
    pub fn collection(&self) -> Option<&str> {
        self.config.collection.as_deref()
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
        let mut search = SearchPointsBuilder::new(collection, query, limit as u64);

        if include_vectors {
            search = search.with_vectors(VectorsSelectorOptions::Enable(true));
        }

        if include_metadata {
            search = search.with_payload(SelectorOptions::Enable(true));
        }

        if let Some(filter_json) = filter
            && let Some(conditions) = parse_filter(filter_json)
        {
            search = search.filter(Filter::must(conditions));
        }

        let response = self
            .client
            .search_points(search)
            .await
            .map_err(|e| Error::provider(e.to_string()))?;

        let results = response
            .result
            .into_iter()
            .map(|point| {
                let id = extract_point_id(point.id).unwrap_or_default();
                let vector = extract_vector(point.vectors);

                let metadata: HashMap<String, serde_json::Value> = point
                    .payload
                    .into_iter()
                    .map(|(k, v)| (k, qdrant_value_to_json(v)))
                    .collect();

                SearchResult {
                    id,
                    score: point.score,
                    vector,
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

impl std::fmt::Debug for QdrantProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QdrantProvider").finish()
    }
}

fn extract_vector(vectors: Option<qdrant_client::qdrant::VectorsOutput>) -> Option<Vec<f32>> {
    use qdrant_client::qdrant::vectors_output::VectorsOptions;

    vectors.and_then(|v| match v.vectors_options {
        #[allow(deprecated)]
        Some(VectorsOptions::Vector(vec)) => Some(vec.data),
        _ => None,
    })
}

fn extract_point_id(id: Option<PointId>) -> Option<String> {
    use qdrant_client::qdrant::point_id::PointIdOptions;

    match id {
        Some(PointId {
            point_id_options: Some(id),
        }) => match id {
            PointIdOptions::Num(n) => Some(n.to_string()),
            PointIdOptions::Uuid(s) => Some(s),
        },
        _ => None,
    }
}

pub(crate) fn json_to_qdrant_value(value: serde_json::Value) -> qdrant_client::qdrant::Value {
    use qdrant_client::qdrant::value::Kind;

    let kind = match value {
        serde_json::Value::Null => Kind::NullValue(0),
        serde_json::Value::Bool(b) => Kind::BoolValue(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Kind::IntegerValue(i)
            } else if let Some(f) = n.as_f64() {
                Kind::DoubleValue(f)
            } else {
                Kind::StringValue(n.to_string())
            }
        }
        serde_json::Value::String(s) => Kind::StringValue(s),
        serde_json::Value::Array(arr) => {
            let values: Vec<qdrant_client::qdrant::Value> =
                arr.into_iter().map(json_to_qdrant_value).collect();
            Kind::ListValue(qdrant_client::qdrant::ListValue { values })
        }
        serde_json::Value::Object(obj) => {
            let fields: HashMap<String, qdrant_client::qdrant::Value> = obj
                .into_iter()
                .map(|(k, v)| (k, json_to_qdrant_value(v)))
                .collect();
            Kind::StructValue(qdrant_client::qdrant::Struct { fields })
        }
    };

    qdrant_client::qdrant::Value { kind: Some(kind) }
}

fn qdrant_value_to_json(value: qdrant_client::qdrant::Value) -> serde_json::Value {
    use qdrant_client::qdrant::value::Kind;

    match value.kind {
        Some(Kind::NullValue(_)) => serde_json::Value::Null,
        Some(Kind::BoolValue(b)) => serde_json::Value::Bool(b),
        Some(Kind::IntegerValue(i)) => serde_json::json!(i),
        Some(Kind::DoubleValue(f)) => serde_json::json!(f),
        Some(Kind::StringValue(s)) => serde_json::Value::String(s),
        Some(Kind::ListValue(list)) => {
            let arr: Vec<serde_json::Value> =
                list.values.into_iter().map(qdrant_value_to_json).collect();
            serde_json::Value::Array(arr)
        }
        Some(Kind::StructValue(obj)) => {
            let map: serde_json::Map<String, serde_json::Value> = obj
                .fields
                .into_iter()
                .map(|(k, v)| (k, qdrant_value_to_json(v)))
                .collect();
            serde_json::Value::Object(map)
        }
        None => serde_json::Value::Null,
    }
}

fn parse_filter(filter: &serde_json::Value) -> Option<Vec<Condition>> {
    if let serde_json::Value::Object(obj) = filter {
        let conditions: Vec<Condition> = obj
            .iter()
            .filter_map(|(key, value)| match value {
                serde_json::Value::String(s) => Some(Condition::matches(key.clone(), s.clone())),
                serde_json::Value::Number(n) => {
                    n.as_i64().map(|i| Condition::matches(key.clone(), i))
                }
                serde_json::Value::Bool(b) => Some(Condition::matches(key.clone(), *b)),
                _ => None,
            })
            .collect();

        if conditions.is_empty() {
            None
        } else {
            Some(conditions)
        }
    } else {
        None
    }
}
