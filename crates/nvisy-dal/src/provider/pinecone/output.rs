//! Pinecone DataOutput implementation.

use async_trait::async_trait;
use pinecone_sdk::models::Vector;

use super::{PineconeProvider, hashmap_to_metadata};
use crate::core::DataOutput;
use crate::datatype::Embedding;
use crate::error::{Error, Result};

#[async_trait]
impl DataOutput for PineconeProvider {
    type Item = Embedding;

    async fn write(&self, items: Vec<Embedding>) -> Result<()> {
        let namespace = self
            .namespace()
            .map(|ns| pinecone_sdk::models::Namespace::from(ns))
            .unwrap_or_default();

        let pinecone_vectors: Vec<Vector> = items
            .into_iter()
            .map(|v| {
                let metadata = if v.metadata.is_empty() {
                    None
                } else {
                    Some(hashmap_to_metadata(v.metadata))
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
            .map_err(|e| Error::provider(e.to_string()))?;

        Ok(())
    }
}
