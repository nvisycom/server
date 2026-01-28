//! Weaviate vector database provider.
//!
//! Provides vector upsert operations for the Weaviate vector database.

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::datatypes::Embedding;
use crate::runtime::{self, PyDataOutput, PyProvider};
use crate::{DataOutput, Provider, Result};

/// Credentials for Weaviate connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct WeaviateCredentials {
    /// Weaviate server URL (e.g., 'http://localhost:8080' or Weaviate Cloud URL).
    pub url: String,
    /// API key for Weaviate Cloud or secured instances.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

/// Parameters for Weaviate operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct WeaviateParams {
    /// Collection name.
    pub collection: String,
}

/// Weaviate provider for vector upsert operations.
pub struct WeaviateProvider {
    inner: PyProvider,
    output: PyDataOutput<Embedding>,
}

#[async_trait::async_trait]
impl Provider for WeaviateProvider {
    type Credentials = WeaviateCredentials;
    type Params = WeaviateParams;

    async fn connect(
        params: Self::Params,
        credentials: Self::Credentials,
    ) -> nvisy_core::Result<Self> {
        let inner = runtime::connect("weaviate", credentials, params).await?;
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
impl DataOutput for WeaviateProvider {
    type Datatype = Embedding;

    async fn write(&self, items: Vec<Self::Datatype>) -> Result<()> {
        self.output.write(items).await
    }
}

impl std::fmt::Debug for WeaviateProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WeaviateProvider").finish_non_exhaustive()
    }
}
