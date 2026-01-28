//! Milvus vector database provider.
//!
//! Provides vector upsert operations for the Milvus vector database.

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::datatypes::Embedding;
use crate::runtime::{self, PyDataOutput, PyProvider};
use crate::{DataOutput, Provider, Result};

/// Credentials for Milvus connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct MilvusCredentials {
    /// Milvus server URI (e.g., 'http://localhost:19530' or Zilliz Cloud URI).
    pub uri: String,
    /// API token for Zilliz Cloud or secured instances.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

/// Parameters for Milvus operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct MilvusParams {
    /// Collection name.
    pub collection: String,
}

/// Milvus provider for vector upsert operations.
pub struct MilvusProvider {
    inner: PyProvider,
    output: PyDataOutput<Embedding>,
}

#[async_trait::async_trait]
impl Provider for MilvusProvider {
    type Credentials = MilvusCredentials;
    type Params = MilvusParams;

    async fn connect(
        params: Self::Params,
        credentials: Self::Credentials,
    ) -> nvisy_core::Result<Self> {
        let inner = runtime::connect("milvus", credentials, params).await?;
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
impl DataOutput for MilvusProvider {
    type Datatype = Embedding;

    async fn write(&self, items: Vec<Self::Datatype>) -> Result<()> {
        self.output.write(items).await
    }
}

impl std::fmt::Debug for MilvusProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MilvusProvider").finish_non_exhaustive()
    }
}
