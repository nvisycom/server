//! Milvus DataOutput implementation.

use async_trait::async_trait;
use milvus::data::FieldColumn;
use milvus::schema::FieldSchema;
use milvus::value::ValueVec;

use super::MilvusProvider;
use crate::core::DataOutput;
use crate::datatype::Embedding;
use crate::error::{Error, Result};

#[async_trait]
impl DataOutput for MilvusProvider {
    type Item = Embedding;

    async fn write(&self, items: Vec<Embedding>) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }

        let collection = self.collection();

        let dim = items.first().map(|v| v.vector.len()).unwrap_or(0);

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
