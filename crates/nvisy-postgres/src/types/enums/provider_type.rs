//! Inference provider-type enumeration.

use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// The inference model type backing a workspace provider.
///
/// Corresponds to the `PROVIDER_TYPE` PostgreSQL enum. A workspace provider is an
/// inference service the platform calls; this says which kind of model it is — a
/// language model for chat, or a named-entity-recognition model for extraction.
/// The concrete vendor (the `provider` column, e.g. `openai`) is orthogonal and
/// stays open; this type is a stable, closed set used to find a workspace's
/// provider of a given type without decrypting its config.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::ProviderType"]
pub enum ProviderType {
    /// A language model (chat / completion).
    #[db_rename = "llm"]
    #[serde(rename = "llm")]
    Llm,

    /// A named-entity-recognition model (entity extraction).
    #[db_rename = "ner"]
    #[serde(rename = "ner")]
    Ner,
}
