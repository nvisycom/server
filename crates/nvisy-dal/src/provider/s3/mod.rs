//! Amazon S3 provider.

mod config;

use async_trait::async_trait;
pub use config::S3Config;
use futures::StreamExt;
use opendal::{Operator, services};

use crate::core::{Context, DataInput, DataOutput, InputStream};
use crate::datatype::Blob;
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

#[async_trait]
impl DataInput<Blob> for S3Provider {
    async fn read(&self, ctx: &Context) -> Result<InputStream<'static, Blob>> {
        let prefix = ctx.target.as_deref().unwrap_or("");
        let limit = ctx.limit.unwrap_or(usize::MAX);

        let lister = self
            .operator
            .lister(prefix)
            .await
            .map_err(|e| Error::provider(e.to_string()))?;

        let operator = self.operator.clone();

        let stream = lister.take(limit).filter_map(move |entry_result| {
            let op = operator.clone();
            async move {
                match entry_result {
                    Ok(entry) => {
                        let path = entry.path().to_string();
                        if path.ends_with('/') {
                            return None;
                        }

                        match op.read(&path).await {
                            Ok(data) => {
                                let mut blob = Blob::new(path.clone(), data.to_bytes());
                                if let Ok(meta) = op.stat(&path).await {
                                    if let Some(ct) = meta.content_type() {
                                        blob = blob.with_content_type(ct);
                                    }
                                }
                                Some(Ok(blob))
                            }
                            Err(e) => Some(Err(Error::provider(e.to_string()))),
                        }
                    }
                    Err(e) => Some(Err(Error::provider(e.to_string()))),
                }
            }
        });

        Ok(InputStream::new(Box::pin(stream)))
    }
}

#[async_trait]
impl DataOutput<Blob> for S3Provider {
    async fn write(&self, _ctx: &Context, items: Vec<Blob>) -> Result<()> {
        for blob in items {
            self.operator
                .write(&blob.path, blob.data)
                .await
                .map_err(|e| Error::provider(e.to_string()))?;
        }
        Ok(())
    }
}

impl std::fmt::Debug for S3Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("S3Provider").finish()
    }
}
