//! Amazon S3 provider.

mod config;
mod input;
mod output;

pub use config::{S3Credentials, S3Params};
use opendal::{Operator, services};

use crate::core::IntoProvider;
use crate::error::Error;

/// Amazon S3 provider for blob storage.
#[derive(Clone)]
pub struct S3Provider {
    operator: Operator,
}

#[async_trait::async_trait]
impl IntoProvider for S3Provider {
    type Credentials = S3Credentials;
    type Params = S3Params;

    async fn create(
        params: Self::Params,
        credentials: Self::Credentials,
    ) -> nvisy_core::Result<Self> {
        let mut builder = services::S3::default()
            .bucket(&params.bucket)
            .region(&credentials.region)
            .access_key_id(&credentials.access_key_id)
            .secret_access_key(&credentials.secret_access_key);

        if let Some(ref endpoint) = credentials.endpoint {
            builder = builder.endpoint(endpoint);
        }

        if let Some(ref prefix) = params.prefix {
            builder = builder.root(prefix);
        }

        let operator = Operator::new(builder)
            .map(|op| op.finish())
            .map_err(|e| Error::connection(e.to_string()))?;

        Ok(Self { operator })
    }
}

impl std::fmt::Debug for S3Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("S3Provider").finish()
    }
}
