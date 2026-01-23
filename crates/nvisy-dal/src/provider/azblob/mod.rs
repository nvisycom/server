//! Azure Blob Storage provider.

mod config;
mod input;
mod output;

pub use config::{AzblobCredentials, AzblobParams};
use opendal::{Operator, services};

use crate::core::IntoProvider;
use crate::error::Error;

/// Azure Blob Storage provider for blob storage.
#[derive(Clone)]
pub struct AzblobProvider {
    operator: Operator,
}

#[async_trait::async_trait]
impl IntoProvider for AzblobProvider {
    type Credentials = AzblobCredentials;
    type Params = AzblobParams;

    async fn create(
        params: Self::Params,
        credentials: Self::Credentials,
    ) -> nvisy_core::Result<Self> {
        let mut builder = services::Azblob::default()
            .account_name(&credentials.account_name)
            .container(&params.container);

        if let Some(ref account_key) = credentials.account_key {
            builder = builder.account_key(account_key);
        }

        if let Some(ref sas_token) = credentials.sas_token {
            builder = builder.sas_token(sas_token);
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

impl std::fmt::Debug for AzblobProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AzblobProvider").finish()
    }
}
