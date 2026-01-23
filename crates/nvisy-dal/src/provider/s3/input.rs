//! S3 DataInput implementation.

use async_trait::async_trait;
use futures::StreamExt;

use super::S3Provider;
use crate::core::{DataInput, InputStream, ObjectContext};
use crate::datatype::Blob;
use crate::error::{Error, Result};

#[async_trait]
impl DataInput for S3Provider {
    type Item = Blob;
    type Context = ObjectContext;

    async fn read(&self, ctx: &ObjectContext) -> Result<InputStream<Blob>> {
        let prefix = ctx.prefix.as_deref().unwrap_or("");
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
                                if let Ok(meta) = op.stat(&path).await
                                    && let Some(ct) = meta.content_type()
                                {
                                    blob = blob.with_content_type(ct);
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
