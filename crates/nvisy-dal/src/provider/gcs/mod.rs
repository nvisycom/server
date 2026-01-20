//! Google Cloud Storage provider.

mod config;

use async_trait::async_trait;
pub use config::GcsConfig;
use futures::StreamExt;
use opendal::{Operator, services};

use crate::core::{Context, DataInput, DataOutput, InputStream};
use crate::datatype::Blob;
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

#[async_trait]
impl DataInput<Blob> for GcsProvider {
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
impl DataOutput<Blob> for GcsProvider {
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

impl std::fmt::Debug for GcsProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GcsProvider").finish()
    }
}
