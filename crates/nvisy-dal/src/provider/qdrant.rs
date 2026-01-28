//! Qdrant vector database provider.
//!
//! Provides vector upsert operations for the Qdrant vector database.

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::datatypes::Embedding;
use crate::runtime::{self, PyDataOutput, PyProvider};
use crate::{DataOutput, Provider, Result};

/// Credentials for Qdrant connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct QdrantCredentials {
    /// Qdrant server URL (e.g., 'http://localhost:6333' or cloud URL).
    pub url: String,
    /// API key for Qdrant Cloud or secured instances.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

/// Parameters for Qdrant operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct QdrantParams {
    /// Collection name.
    pub collection: String,
}

/// Qdrant provider for vector upsert operations.
pub struct QdrantProvider {
    inner: PyProvider,
    output: PyDataOutput<Embedding>,
}

#[async_trait::async_trait]
impl Provider for QdrantProvider {
    type Credentials = QdrantCredentials;
    type Params = QdrantParams;

    async fn connect(
        params: Self::Params,
        credentials: Self::Credentials,
    ) -> nvisy_core::Result<Self> {
        let inner = runtime::connect("qdrant", credentials, params).await?;
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
impl DataOutput for QdrantProvider {
    type Datatype = Embedding;

    async fn write(&self, items: Vec<Self::Datatype>) -> Result<()> {
        self.output.write(items).await
    }
}

impl std::fmt::Debug for QdrantProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QdrantProvider").finish_non_exhaustive()
    }
}
