//! Pinecone vector database provider.
//!
//! Provides vector upsert operations for the Pinecone vector database.

use serde::{Deserialize, Serialize};

use crate::datatypes::Embedding;
use crate::runtime::{self, PyDataOutput, PyProvider};
use crate::{DataOutput, Provider, Result};

/// Credentials for Pinecone connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PineconeCredentials {
    /// Pinecone API key.
    pub api_key: String,
}

/// Parameters for Pinecone operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PineconeParams {
    /// Index name.
    pub index_name: String,
    /// Namespace within the index.
    pub namespace: String,
}

/// Pinecone provider for vector upsert operations.
pub struct PineconeProvider {
    inner: PyProvider,
    output: PyDataOutput<Embedding>,
}

#[async_trait::async_trait]
impl Provider for PineconeProvider {
    type Credentials = PineconeCredentials;
    type Params = PineconeParams;

    async fn connect(
        params: Self::Params,
        credentials: Self::Credentials,
    ) -> nvisy_core::Result<Self> {
        let inner = runtime::connect("pinecone", credentials, params).await?;
        Ok(Self {
            output: inner.as_data_output(),
            inner,
        })
    }

    async fn disconnect(self) -> nvisy_core::Result<()> {
        self.inner.disconnect().await.map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl DataOutput for PineconeProvider {
    type Datatype = Embedding;

    async fn write(&self, items: Vec<Self::Datatype>) -> Result<()> {
        self.output.write(items).await
    }
}

impl std::fmt::Debug for PineconeProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PineconeProvider").finish_non_exhaustive()
    }
}
