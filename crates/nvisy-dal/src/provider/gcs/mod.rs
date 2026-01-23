//! Google Cloud Storage provider.

mod config;
mod input;
mod output;

pub use config::{GcsCredentials, GcsParams};
use opendal::{Operator, services};

use crate::core::IntoProvider;
use crate::error::Error;

/// Google Cloud Storage provider for blob storage.
#[derive(Clone)]
pub struct GcsProvider {
    operator: Operator,
}

#[async_trait::async_trait]
impl IntoProvider for GcsProvider {
    type Credentials = GcsCredentials;
    type Params = GcsParams;

    async fn create(
        params: Self::Params,
        credentials: Self::Credentials,
    ) -> nvisy_core::Result<Self> {
        let mut builder = services::Gcs::default()
            .bucket(&params.bucket)
            .credential(&credentials.credentials_json);

        if let Some(ref prefix) = params.prefix {
            builder = builder.root(prefix);
        }

        let operator = Operator::new(builder)
            .map(|op| op.finish())
            .map_err(|e| Error::connection(e.to_string()))?;

        Ok(Self { operator })
    }
}

impl std::fmt::Debug for GcsProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GcsProvider").finish()
    }
}
