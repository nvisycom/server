//! Qdrant vector store backend.

mod backend;
mod config;

pub use backend::QdrantBackend;
pub use config::QdrantConfig;
