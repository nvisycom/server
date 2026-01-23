//! Google Cloud Storage provider.

mod config;
mod input;
mod output;

pub use config::GcsConfig;

use opendal::{Operator, services};

use crate::error::{Error, Result};

/// Google Cloud Storage provider for blob storage.
#[derive(Clone)]
pub struct GcsProvider {
    operator: Operator,
}

impl GcsProvider {
    /// Creates a new GCS provider.
    pub fn new(config: &GcsConfig) -> Result<Self> {
        let mut builder = services::Gcs::default().bucket(&config.bucket);

        if let Some(ref credentials) = config.credentials {
            builder = builder.credential(credentials);
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

impl std::fmt::Debug for GcsProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GcsProvider").finish()
    }
}
