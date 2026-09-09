//! Inference provider-type enumeration.

use super::db_enum;

db_enum! {
    /// The inference model type backing a workspace provider.
    ///
    /// Corresponds to the `PROVIDER_TYPE` PostgreSQL enum. A workspace provider is
    /// an inference service the platform calls; this says which kind of model it
    /// is — a language model for chat, or a named-entity-recognition model for
    /// extraction. The concrete vendor (the `provider` column, e.g. `openai`) is
    /// orthogonal and stays open; this type is a stable, closed set used to find a
    /// workspace's provider of a given type without decrypting its config.
    pub enum ProviderType = "crate::schema::sql_types::ProviderType" {
        /// A language model (chat / completion).
        Llm = "llm",
        /// A named-entity-recognition model (entity extraction).
        Ner = "ner",
    }
}
