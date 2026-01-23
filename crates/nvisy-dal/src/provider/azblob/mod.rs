//! Azure Blob Storage provider.

mod config;
mod input;
mod output;

pub use config::AzblobConfig;

use opendal::{Operator, services};

use crate::error::{Error, Result};

/// Azure Blob Storage provider for blob storage.
#[derive(Clone)]
pub struct AzblobProvider {
    operator: Operator,
}

impl AzblobProvider {
    /// Creates a new Azure Blob provider.
    pub fn new(config: &AzblobConfig) -> Result<Self> {
        let mut builder = services::Azblob::default()
            .account_name(&config.account_name)
            .container(&config.container);

        if let Some(ref account_key) = config.account_key {
            builder = builder.account_key(account_key);
        }

        if let Some(ref sas_token) = config.sas_token {
            builder = builder.sas_token(sas_token);
        }

        if let Some(ref prefix) = config.prefix {
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
