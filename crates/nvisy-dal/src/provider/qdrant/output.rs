//! Qdrant DataOutput implementation.

use std::collections::HashMap;

use async_trait::async_trait;
use qdrant_client::qdrant::{PointStruct, UpsertPointsBuilder};

use super::{QdrantProvider, json_to_qdrant_value};
use crate::core::DataOutput;
use crate::datatype::Embedding;
use crate::error::{Error, Result};

#[async_trait]
impl DataOutput for QdrantProvider {
    type Item = Embedding;

    async fn write(&self, items: Vec<Embedding>) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }

        let collection = self
            .collection()
            .ok_or_else(|| Error::invalid_input("Collection name required in provider config"))?;

        let dimensions = items
            .first()
            .map(|v| v.vector.len())
            .ok_or_else(|| Error::invalid_input("No embeddings provided"))?;

        self.ensure_collection(collection, dimensions).await?;

        let points: Vec<PointStruct> = items
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
            .upsert_points(UpsertPointsBuilder::new(collection, points))
            .await
            .map_err(|e| Error::provider(e.to_string()))?;

        Ok(())
    }
}
