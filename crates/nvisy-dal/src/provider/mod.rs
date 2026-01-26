//! Provider implementations for external services.
//!
//! Each provider module exports credentials and params types
//! along with the main provider struct.
//!
//! Data types for input/output are in the `datatype` module:
//! - `Record` for PostgreSQL rows
//! - `Object` for S3 objects
//! - `Embedding` for Pinecone vectors
//!
//! Context types for pagination are in the `core` module:
//! - `RelationalContext` for relational databases
//! - `ObjectContext` for object storage
//! - `VectorContext` for vector databases
//!
//! Available providers:
//! - `postgres`: PostgreSQL relational database
//! - `s3`: AWS S3 / MinIO object storage
//! - `pinecone`: Pinecone vector database

mod pinecone;
mod postgres;
mod s3;

pub use self::pinecone::{PineconeCredentials, PineconeParams, PineconeProvider};
pub use self::postgres::{PostgresCredentials, PostgresParams, PostgresProvider};
pub use self::s3::{S3Credentials, S3Params, S3Provider};
