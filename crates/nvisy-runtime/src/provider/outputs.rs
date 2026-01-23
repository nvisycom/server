//! Output provider types and implementations.

use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context as TaskContext, Poll};

use derive_more::From;
use futures::Sink;
use nvisy_dal::provider::{
    AzblobProvider, GcsProvider, MilvusProvider, MysqlProvider, PgVectorProvider, PineconeProvider,
    PostgresProvider, QdrantProvider, S3Provider,
};
use nvisy_dal::{AnyDataValue, DataTypeId};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;

use super::ProviderCredentials;
use super::backend::{
    AzblobParams, GcsParams, IntoProvider, MilvusParams, MysqlParams, PgVectorParams,
    PineconeParams, PostgresParams, QdrantParams, S3Params,
};
use crate::error::{Error, Result};
use crate::graph::DataSink;

/// Output provider parameters (storage backends + vector DBs).
#[derive(Debug, Clone, PartialEq, From, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OutputProviderParams {
    /// Amazon S3 storage.
    S3(S3Params),
    /// Google Cloud Storage.
    Gcs(GcsParams),
    /// Azure Blob Storage.
    Azblob(AzblobParams),
    /// PostgreSQL database.
    Postgres(PostgresParams),
    /// MySQL database.
    Mysql(MysqlParams),
    /// Qdrant vector database.
    Qdrant(QdrantParams),
    /// Pinecone vector database.
    Pinecone(PineconeParams),
    /// Milvus vector database.
    Milvus(MilvusParams),
    /// pgvector (PostgreSQL extension).
    PgVector(PgVectorParams),
}

impl OutputProviderParams {
    /// Returns the credentials ID for this provider.
    pub fn credentials_id(&self) -> Uuid {
        match self {
            Self::S3(p) => p.credentials_id,
            Self::Gcs(p) => p.credentials_id,
            Self::Azblob(p) => p.credentials_id,
            Self::Postgres(p) => p.credentials_id,
            Self::Mysql(p) => p.credentials_id,
            Self::Qdrant(p) => p.credentials_id,
            Self::Pinecone(p) => p.credentials_id,
            Self::Milvus(p) => p.credentials_id,
            Self::PgVector(p) => p.credentials_id,
        }
    }

    /// Returns the provider kind as a string.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::S3(_) => "s3",
            Self::Gcs(_) => "gcs",
            Self::Azblob(_) => "azblob",
            Self::Postgres(_) => "postgres",
            Self::Mysql(_) => "mysql",
            Self::Qdrant(_) => "qdrant",
            Self::Pinecone(_) => "pinecone",
            Self::Milvus(_) => "milvus",
            Self::PgVector(_) => "pgvector",
        }
    }

    /// Returns the output data type for this provider.
    pub const fn output_type(&self) -> DataTypeId {
        match self {
            Self::S3(_) | Self::Gcs(_) | Self::Azblob(_) => DataTypeId::Blob,
            Self::Postgres(_) | Self::Mysql(_) => DataTypeId::Record,
            Self::Qdrant(_) | Self::Pinecone(_) | Self::Milvus(_) | Self::PgVector(_) => {
                DataTypeId::Embedding
            }
        }
    }
}

#[async_trait::async_trait]
impl IntoProvider for OutputProviderParams {
    type Credentials = ProviderCredentials;
    type Output = OutputProvider;

    async fn into_provider(self, credentials: Self::Credentials) -> Result<Self::Output> {
        match (self, credentials) {
            (Self::S3(p), ProviderCredentials::S3(c)) => {
                Ok(OutputProvider::S3(p.into_provider(c).await?))
            }
            (Self::Gcs(p), ProviderCredentials::Gcs(c)) => {
                Ok(OutputProvider::Gcs(p.into_provider(c).await?))
            }
            (Self::Azblob(p), ProviderCredentials::Azblob(c)) => {
                Ok(OutputProvider::Azblob(p.into_provider(c).await?))
            }
            (Self::Postgres(p), ProviderCredentials::Postgres(c)) => {
                Ok(OutputProvider::Postgres(p.into_provider(c).await?))
            }
            (Self::Mysql(p), ProviderCredentials::Mysql(c)) => {
                Ok(OutputProvider::Mysql(p.into_provider(c).await?))
            }
            (Self::Qdrant(p), ProviderCredentials::Qdrant(c)) => {
                Ok(OutputProvider::Qdrant(p.into_provider(c).await?))
            }
            (Self::Pinecone(p), ProviderCredentials::Pinecone(c)) => {
                Ok(OutputProvider::Pinecone(p.into_provider(c).await?))
            }
            (Self::Milvus(p), ProviderCredentials::Milvus(c)) => {
                Ok(OutputProvider::Milvus(p.into_provider(c).await?))
            }
            (Self::PgVector(p), ProviderCredentials::PgVector(c)) => {
                Ok(OutputProvider::PgVector(p.into_provider(c).await?))
            }
            (params, creds) => Err(Error::Internal(format!(
                "credentials type mismatch: expected '{}', got '{}'",
                params.kind(),
                creds.kind()
            ))),
        }
    }
}

/// Output provider instance (created from config).
#[derive(Debug)]
pub enum OutputProvider {
    S3(S3Provider),
    Gcs(GcsProvider),
    Azblob(AzblobProvider),
    Postgres(PostgresProvider),
    Mysql(MysqlProvider),
    Qdrant(QdrantProvider),
    Pinecone(PineconeProvider),
    Milvus(MilvusProvider),
    PgVector(PgVectorProvider),
}

impl OutputProvider {
    /// Returns the input data type expected by this provider.
    pub const fn input_type(&self) -> DataTypeId {
        match self {
            Self::S3(_) | Self::Gcs(_) | Self::Azblob(_) => DataTypeId::Blob,
            Self::Postgres(_) | Self::Mysql(_) => DataTypeId::Record,
            Self::Qdrant(_) | Self::Pinecone(_) | Self::Milvus(_) | Self::PgVector(_) => {
                DataTypeId::Embedding
            }
        }
    }

    /// Creates a sink for streaming writes to the provider.
    ///
    /// The sink buffers items and writes them on flush/close.
    pub fn write_sink(self) -> DataSink {
        let sink = ProviderSink::new(self);
        Box::pin(sink)
    }

    /// Writes data to the provider, accepting type-erased values.
    pub async fn write(&self, data: Vec<AnyDataValue>) -> Result<()> {
        match self {
            Self::S3(p) => write_data!(p, data, Blob, into_blob),
            Self::Gcs(p) => write_data!(p, data, Blob, into_blob),
            Self::Azblob(p) => write_data!(p, data, Blob, into_blob),
            Self::Postgres(p) => write_data!(p, data, Record, into_record),
            Self::Mysql(p) => write_data!(p, data, Record, into_record),
            Self::Qdrant(p) => write_data!(p, data, Embedding, into_embedding),
            Self::Pinecone(p) => write_data!(p, data, Embedding, into_embedding),
            Self::Milvus(p) => write_data!(p, data, Embedding, into_embedding),
            Self::PgVector(p) => write_data!(p, data, Embedding, into_embedding),
        }
    }
}

/// A sink that buffers items and writes them to an output provider.
struct ProviderSink {
    provider: Arc<OutputProvider>,
    buffer: Arc<Mutex<Vec<AnyDataValue>>>,
    flush_future: Option<Pin<Box<dyn std::future::Future<Output = Result<()>> + Send>>>,
}

impl ProviderSink {
    fn new(provider: OutputProvider) -> Self {
        Self {
            provider: Arc::new(provider),
            buffer: Arc::new(Mutex::new(Vec::new())),
            flush_future: None,
        }
    }
}

impl Sink<AnyDataValue> for ProviderSink {
    type Error = Error;

    fn poll_ready(
        self: Pin<&mut Self>,
        _cx: &mut TaskContext<'_>,
    ) -> Poll<std::result::Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(
        self: Pin<&mut Self>,
        item: AnyDataValue,
    ) -> std::result::Result<(), Self::Error> {
        let buffer = self.buffer.clone();
        if let Ok(mut guard) = buffer.try_lock() {
            guard.push(item);
            Ok(())
        } else {
            Err(Error::Internal("buffer lock contention".into()))
        }
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
    ) -> Poll<std::result::Result<(), Self::Error>> {
        if let Some(ref mut future) = self.flush_future {
            return match future.as_mut().poll(cx) {
                Poll::Ready(result) => {
                    self.flush_future = None;
                    Poll::Ready(result)
                }
                Poll::Pending => Poll::Pending,
            };
        }

        let buffer = self.buffer.clone();
        let provider = self.provider.clone();

        let future = Box::pin(async move {
            let items = {
                let mut guard = buffer.lock().await;
                std::mem::take(&mut *guard)
            };

            if items.is_empty() {
                return Ok(());
            }

            provider.write(items).await
        });

        self.flush_future = Some(future);
        self.poll_flush(cx)
    }

    fn poll_close(
        self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
    ) -> Poll<std::result::Result<(), Self::Error>> {
        self.poll_flush(cx)
    }
}

/// Helper macro to write data to a provider from AnyDataValue.
macro_rules! write_data {
    ($provider:expr, $data:expr, $type:ident, $converter:ident) => {{
        use nvisy_dal::core::DataOutput;
        use nvisy_dal::datatype::$type;

        let items: Vec<$type> = $data.into_iter().filter_map(|v| v.$converter()).collect();

        $provider
            .write(items)
            .await
            .map_err(|e| Error::Internal(e.to_string()))
    }};
}

use write_data;
