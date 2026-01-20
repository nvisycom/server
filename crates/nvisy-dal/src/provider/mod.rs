//! Data providers for various storage backends.

// Storage providers (OpenDAL-based)
mod azblob;
mod gcs;
mod s3;

// Database providers (OpenDAL-based)
mod mysql;
mod postgres;

// Vector providers
mod milvus;
mod pgvector;
mod pinecone;
mod qdrant;

mod config;

// Re-export storage providers
pub use azblob::{AzblobConfig, AzblobProvider};
// Re-export unified config
pub use config::ProviderConfig;
pub use gcs::{GcsConfig, GcsProvider};
// Re-export vector providers
pub use milvus::{MilvusConfig, MilvusProvider};
// Re-export database providers
pub use mysql::{MysqlConfig, MysqlProvider};
pub use pgvector::{DistanceMetric, IndexType, PgVectorConfig, PgVectorProvider};
pub use pinecone::{PineconeConfig, PineconeProvider};
pub use postgres::{PostgresConfig, PostgresProvider};
pub use qdrant::{QdrantConfig, QdrantProvider};
pub use s3::{S3Config, S3Provider};
