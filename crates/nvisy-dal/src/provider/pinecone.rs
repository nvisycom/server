//! Pinecone vector database provider.
//!
//! Provides vector upsert operations for the Pinecone vector database.

use serde::{Deserialize, Serialize};

use crate::Result;
use crate::core::{DataOutput, Provider};
use crate::datatype::Embedding;
use crate::python::{PyDataOutput, PyProvider, PyProviderLoader};

/// Credentials for Pinecone connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PineconeCredentials {
    /// Pinecone API key.
    pub api_key: String,
}

/// Parameters for Pinecone operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl PineconeProvider {
    /// Disconnects from Pinecone.
    pub async fn disconnect(self) -> Result<()> {
        self.inner.disconnect().await
    }
}

#[async_trait::async_trait]
impl Provider for PineconeProvider {
    type Credentials = PineconeCredentials;
    type Params = PineconeParams;

    async fn connect(
        params: Self::Params,
        credentials: Self::Credentials,
    ) -> nvisy_core::Result<Self> {
        let loader = PyProviderLoader::new().map_err(crate::Error::from)?;
        let creds_json = serde_json::to_value(&credentials).map_err(crate::Error::from)?;
        let params_json = serde_json::to_value(&params).map_err(crate::Error::from)?;

        let inner = loader
            .load("pinecone", creds_json, params_json)
            .await
            .map_err(crate::Error::from)?;
        let output = PyDataOutput::new(PyProvider::new(inner.clone_py_object()));

        Ok(Self { inner, output })
    }
}

#[async_trait::async_trait]
impl DataOutput for PineconeProvider {
    type Item = Embedding;

    async fn write(&self, items: Vec<Self::Item>) -> Result<()> {
        self.output.write(items).await
    }
}

impl std::fmt::Debug for PineconeProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PineconeProvider").finish_non_exhaustive()
    }
}
