//! Amazon S3 provider.

mod config;
mod input;
mod output;

pub use config::S3Config;

use opendal::{Operator, services};

use crate::error::{Error, Result};

/// Amazon S3 provider for blob storage.
#[derive(Clone)]
pub struct S3Provider {
    operator: Operator,
}

impl S3Provider {
    /// Creates a new S3 provider.
    pub fn new(config: &S3Config) -> Result<Self> {
        let mut builder = services::S3::default()
            .bucket(&config.bucket)
            .region(&config.region);

        if let Some(ref endpoint) = config.endpoint {
            builder = builder.endpoint(endpoint);
        }

        if let Some(ref access_key_id) = config.access_key_id {
            builder = builder.access_key_id(access_key_id);
        }

        if let Some(ref secret_access_key) = config.secret_access_key {
            builder = builder.secret_access_key(secret_access_key);
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

impl std::fmt::Debug for S3Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("S3Provider").finish()
    }
}
