//! Input node definition types.

use nvisy_dal::provider::{AnyParams, PostgresParams, S3Params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::route::CacheSlot;

/// Input node definition - source of data for the workflow.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum Input {
    /// Read from a storage provider.
    Provider(ProviderInput),
    /// Read from named cache slot (resolved at compile time).
    CacheSlot(CacheSlot),
}

/// Provider-based input configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderInput {
    /// Credentials ID for the provider.
    pub credentials_id: Uuid,
    /// Provider-specific parameters.
    #[serde(flatten)]
    pub params: InputParams,
}

/// Type-erased parameters for input providers.
///
/// Only includes providers that support reading data (DataInput).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "provider", content = "params", rename_all = "snake_case")]
pub enum InputParams {
    /// PostgreSQL parameters.
    Postgres(PostgresParams),
    /// S3 parameters.
    S3(S3Params),
}

impl From<InputParams> for AnyParams {
    fn from(params: InputParams) -> Self {
        match params {
            InputParams::Postgres(p) => AnyParams::Postgres(p),
            InputParams::S3(p) => AnyParams::S3(p),
        }
    }
}
