//! S3 provider.
//!
//! Provides object storage operations for AWS S3 and S3-compatible services.

use serde::{Deserialize, Serialize};

use crate::Result;
use crate::core::{DataInput, DataOutput, InputStream, ObjectContext, Provider};
use crate::datatype::Object;
use crate::python::{PyDataInput, PyDataOutput, PyProvider, PyProviderLoader};

/// Credentials for S3 connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3Credentials {
    /// AWS access key ID.
    pub access_key_id: String,
    /// AWS secret access key.
    pub secret_access_key: String,
    /// AWS region.
    pub region: String,
    /// Custom endpoint URL (for MinIO, LocalStack, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint_url: Option<String>,
}

/// Parameters for S3 operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3Params {
    /// Target bucket name.
    pub bucket: String,
    /// Key prefix for all operations.
    pub prefix: String,
}

/// S3 provider for object storage operations.
pub struct S3Provider {
    inner: PyProvider,
    input: PyDataInput<Object, ObjectContext>,
    output: PyDataOutput<Object>,
}

impl S3Provider {
    /// Disconnects from S3.
    pub async fn disconnect(self) -> Result<()> {
        self.inner.disconnect().await
    }
}

#[async_trait::async_trait]
impl Provider for S3Provider {
    type Credentials = S3Credentials;
    type Params = S3Params;

    async fn connect(
        params: Self::Params,
        credentials: Self::Credentials,
    ) -> nvisy_core::Result<Self> {
        let loader = PyProviderLoader::new().map_err(crate::Error::from)?;
        let creds_json = serde_json::to_value(&credentials).map_err(crate::Error::from)?;
        let params_json = serde_json::to_value(&params).map_err(crate::Error::from)?;

        let inner = loader
            .load("s3", creds_json, params_json)
            .await
            .map_err(crate::Error::from)?;
        let input = PyDataInput::new(PyProvider::new(inner.clone_py_object()));
        let output = PyDataOutput::new(PyProvider::new(inner.clone_py_object()));

        Ok(Self {
            inner,
            input,
            output,
        })
    }
}

#[async_trait::async_trait]
impl DataInput for S3Provider {
    type Item = Object;
    type Context = ObjectContext;

    async fn read(&self, ctx: &Self::Context) -> Result<InputStream<Self::Item>> {
        self.input.read(ctx).await
    }
}

#[async_trait::async_trait]
impl DataOutput for S3Provider {
    type Item = Object;

    async fn write(&self, items: Vec<Self::Item>) -> Result<()> {
        self.output.write(items).await
    }
}

impl std::fmt::Debug for S3Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("S3Provider").finish_non_exhaustive()
    }
}
