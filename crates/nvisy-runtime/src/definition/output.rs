//! Output node definition types.

use nvisy_dal::provider::{PineconeParams, PostgresParams, S3Params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::route::CacheSlot;

/// Output node definition - destination for workflow data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "target", rename_all = "snake_case")]
pub enum Output {
    /// Write to a storage provider.
    Provider(ProviderOutput),
    /// Write to named cache slot (resolved at compile time).
    CacheSlot(CacheSlot),
}

/// Provider-based output configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderOutput {
    /// Credentials ID for the provider.
    pub credentials_id: Uuid,
    /// Provider-specific parameters.
    #[serde(flatten)]
    pub params: OutputParams,
}

/// Type-erased parameters for output providers.
///
/// Includes all providers that support writing data (DataOutput).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "provider", content = "params", rename_all = "snake_case")]
pub enum OutputParams {
    /// PostgreSQL parameters.
    Postgres(PostgresParams),
    /// S3 parameters.
    S3(S3Params),
    /// Pinecone parameters.
    Pinecone(PineconeParams),
}
