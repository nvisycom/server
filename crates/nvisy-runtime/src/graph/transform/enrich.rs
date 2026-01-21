//! Enrich transformer configuration.

use nvisy_rig::provider::CompletionModel;
use serde::{Deserialize, Serialize};

/// Configuration for enriching data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnrichConfig {
    /// Model to use for enrichment.
    #[serde(flatten)]
    pub model: CompletionModel,
    /// Prompt template for enrichment.
    pub prompt: String,
}
