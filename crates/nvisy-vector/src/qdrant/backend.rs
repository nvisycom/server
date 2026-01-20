//! Qdrant backend implementation.

use std::collections::HashMap;

use async_trait::async_trait;
use nvisy_data::{
    DataError, DataResult, VectorContext, VectorData, VectorOutput, VectorSearchOptions,
    VectorSearchResult,
};
use qdrant_client::Qdrant;
use qdrant_client::qdrant::vectors_config::Config as VectorsConfig;
use qdrant_client::qdrant::with_payload_selector::SelectorOptions;
use qdrant_client::qdrant::with_vectors_selector::SelectorOptions as VectorsSelectorOptions;
use qdrant_client::qdrant::{
    Condition, CreateCollectionBuilder, Distance, Filter, PointId, PointStruct,
    SearchPointsBuilder, UpsertPointsBuilder, VectorParamsBuilder,
};

use super::QdrantConfig;
use crate::TRACING_TARGET;

/// Qdrant backend implementation.
pub struct QdrantBackend {
    client: Qdrant,
    #[allow(dead_code)]
    config: QdrantConfig,
}

impl QdrantBackend {
    /// Creates a new Qdrant backend.
    pub async fn new(config: &QdrantConfig) -> DataResult<Self> {
        let client = Qdrant::from_url(&config.url)
            .api_key(config.api_key.clone())
            .build()
            .map_err(|e| DataError::connection(e.to_string()))?;

        tracing::debug!(
            target: TRACING_TARGET,
            url = %config.url,
            "Connected to Qdrant"
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
            .collection_exists(name)
            .await
            .map_err(|e| DataError::backend(e.to_string()))?;

        if !exists {
            let vectors_config = VectorsConfig::Params(
                VectorParamsBuilder::new(dimensions as u64, Distance::Cosine).build(),
            );

            self.client
                .create_collection(
                    CreateCollectionBuilder::new(name).vectors_config(vectors_config),
                )
                .await
                .map_err(|e| DataError::backend(e.to_string()))?;

            tracing::info!(
                target: TRACING_TARGET,
                collection = %name,
                dimensions = %dimensions,
                "Created Qdrant collection"
            );
        }

        Ok(())
    }

    /// Extracts vector data from Qdrant's VectorsOutput.
    fn extract_vector(vectors: Option<qdrant_client::qdrant::VectorsOutput>) -> Option<Vec<f32>> {
        use qdrant_client::qdrant::vectors_output::VectorsOptions;

        vectors.and_then(|v| match v.vectors_options {
            #[allow(deprecated)]
            Some(VectorsOptions::Vector(vec)) => Some(vec.data),
            _ => None,
        })
    }

    /// Extracts point ID as a string.
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
}

#[async_trait]
impl VectorOutput for QdrantBackend {
    async fn insert(&self, ctx: &VectorContext, vectors: Vec<VectorData>) -> DataResult<()> {
        if vectors.is_empty() {
            return Ok(());
        }

        // Get dimensions from the first vector
        let dimensions = vectors
            .first()
            .map(|v| v.vector.len())
            .ok_or_else(|| DataError::invalid("No vectors provided"))?;

        // Ensure collection exists
        self.ensure_collection(&ctx.collection, dimensions).await?;

        let points: Vec<PointStruct> = vectors
            .into_iter()
            .map(|v| {
                let payload: HashMap<String, qdrant_client::qdrant::Value> = v
                    .metadata
                    .into_iter()
                    .map(|(k, v)| (k, json_to_qdrant_value(v)))
                    .collect();

                PointStruct::new(v.id, v.vector, payload)
            })
            .collect();

        self.client
            .upsert_points(UpsertPointsBuilder::new(&ctx.collection, points))
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
        let mut search = SearchPointsBuilder::new(&ctx.collection, query, limit as u64);

        if options.include_vectors {
            search = search.with_vectors(VectorsSelectorOptions::Enable(true));
        }

        if options.include_metadata {
            search = search.with_payload(SelectorOptions::Enable(true));
        }

        if let Some(filter_json) = options.filter
            && let Some(conditions) = parse_filter(&filter_json)
        {
            search = search.filter(Filter::must(conditions));
        }

        let response = self
            .client
            .search_points(search)
            .await
            .map_err(|e| DataError::backend(e.to_string()))?;

        let results = response
            .result
            .into_iter()
            .map(|point| {
                let id = Self::extract_point_id(point.id).unwrap_or_default();
                let vector = Self::extract_vector(point.vectors);

                let metadata: HashMap<String, serde_json::Value> = point
                    .payload
                    .into_iter()
                    .map(|(k, v)| (k, qdrant_value_to_json(v)))
                    .collect();

                VectorSearchResult {
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

/// Converts JSON value to Qdrant value.
fn json_to_qdrant_value(value: serde_json::Value) -> qdrant_client::qdrant::Value {
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

/// Converts Qdrant value to JSON value.
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

/// Parses a JSON filter into Qdrant conditions.
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
