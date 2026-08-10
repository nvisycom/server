//! Deployment catalog response types.

use nvisy_engine::RegisteredRecognizer;
use schemars::JsonSchema;
use serde::Serialize;

/// The engine's registered recognizers, grouped by kind.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecognizerCatalog {
    /// NER (named-entity recognition) recognizers.
    pub ner: Vec<RegisteredRecognizer>,
    /// LLM recognizers.
    pub llm: Vec<RegisteredRecognizer>,
}
