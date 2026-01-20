//! PostgreSQL pgvector backend.

mod backend;
mod config;

pub use backend::PgVectorBackend;
pub use config::{PgVectorConfig, PgVectorDistanceMetric, PgVectorIndexType};
