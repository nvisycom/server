//! S3 provider.
//!
//! Provides object storage operations for AWS S3 and S3-compatible services.

use serde::{Deserialize, Serialize};

use crate::contexts::ObjectContext;
use crate::datatypes::Object;
use crate::params::ObjectParams;
use crate::runtime::{self, PyDataInput, PyDataOutput, PyProvider};
use crate::streams::InputStream;
use crate::{DataInput, DataOutput, Provider, Result, Resumable};

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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct S3Params {
    /// Default content type for uploaded objects.
    #[serde(default = "default_content_type")]
    pub content_type: String,
    /// Object storage parameters (bucket, prefix, batch_size).
    #[serde(flatten)]
    pub object: ObjectParams,
}

fn default_content_type() -> String {
    "application/octet-stream".to_string()
}

/// S3 provider for object storage operations.
pub struct S3Provider {
    inner: PyProvider,
    input: PyDataInput<Object, ObjectContext>,
    output: PyDataOutput<Object>,
}

#[async_trait::async_trait]
impl Provider for S3Provider {
    type Credentials = S3Credentials;
    type Params = S3Params;

    async fn connect(
        params: Self::Params,
        credentials: Self::Credentials,
    ) -> nvisy_core::Result<Self> {
        let inner = runtime::connect("s3", credentials, params).await?;
        Ok(Self {
            input: inner.as_data_input(),
            output: inner.as_data_output(),
            inner,
        })
    }

    async fn disconnect(self) -> nvisy_core::Result<()> {
        self.inner.disconnect().await.map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl DataInput for S3Provider {
    type Context = ObjectContext;
    type Datatype = Object;

    async fn read(
        &self,
        ctx: &Self::Context,
    ) -> Result<InputStream<Resumable<Self::Datatype, Self::Context>>> {
        self.input.read(ctx).await
    }
}

#[async_trait::async_trait]
impl DataOutput for S3Provider {
    type Datatype = Object;

    async fn write(&self, items: Vec<Self::Datatype>) -> Result<()> {
        self.output.write(items).await
    }
}

impl std::fmt::Debug for S3Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("S3Provider").finish_non_exhaustive()
    }
}
