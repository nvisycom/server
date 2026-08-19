//! Provider capability-category enumeration.

use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// The capability category of a connection's provider.
///
/// Corresponds to the `PROVIDER_TYPE` PostgreSQL enum. A stable, closed set:
/// the concrete provider (the `provider` column, e.g. `s3` or `anthropic`) stays
/// open and extensible, while its capability is one of these types. Lets a
/// connection be found by what it can do — e.g. a workspace's language model —
/// without decrypting its config.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::ProviderType"]
pub enum ProviderType {
    /// External object storage (s3, azure, gcs, ...).
    #[db_rename = "object_store"]
    #[serde(rename = "object_store")]
    ObjectStore,

    /// LLM inference (openai, ollama, anthropic, ...).
    #[db_rename = "language_model"]
    #[serde(rename = "language_model")]
    LanguageModel,
}
