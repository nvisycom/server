//! Deployment engine configuration file.
//!
//! The operator-owned TOML file (pointed at by `ENGINE_CONFIG_FILEPATH`) that
//! configures the engine at startup: the NER/LLM recognizer lineups plus the
//! server-wide enrichment and deduplication-calibration defaults. These are
//! uniform across the deployment; a pipeline's definition carries detection
//! intent, not this infrastructure config.

use std::collections::HashMap;
use std::path::Path;

use nvisy_engine::entity::ConfidenceThreshold;
use nvisy_engine::plan::{EnricherParams, MergingStrategyParams, TiebreakerParams};
use nvisy_engine::provider::{LlmConfig, LlmRecognizer, NerConfig, NerRecognizer};
use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// The deployment engine configuration, as loaded from the config file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct EngineFile {
    /// Deployment recognizer lineups, keyed by kind (`recognizers.ner`,
    /// `recognizers.llm`) to mirror a pipeline's `recognizers` selectors.
    #[serde(default)]
    recognizers: EngineRecognizers,
    /// Server-wide enrichment applied uniformly to every run.
    #[serde(default)]
    enrichers: EnricherParams,
    /// Server-wide deduplication defaults (calibration + the fallback merging /
    /// tiebreaker / min-confidence a pipeline inherits when it omits them).
    #[serde(default)]
    deduplication: DeduplicationFile,
}

/// The deployment's recognizer lineups.
///
/// Each is a flat list of recognizer instances the operator wired up; a
/// pipeline's `recognizers.ner` / `.llm` selector then picks which of them run.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EngineRecognizers {
    /// NER recognizer lineup (runs when a pipeline enables `recognizers.ner`).
    #[serde(default)]
    ner: Vec<NerRecognizer>,
    /// LLM recognizer lineup (runs when a pipeline enables `recognizers.llm`).
    #[serde(default)]
    llm: Vec<LlmRecognizer>,
}

/// Server-wide deduplication configuration.
///
/// `calibration` is operator-only (a pipeline never sets it); the other fields
/// are deployment defaults a pipeline inherits per-field when it omits them.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeduplicationFile {
    /// Per-recognizer confidence weights fed to the calibrate layer.
    #[serde(default)]
    calibration: HashMap<String, f64>,
    /// Default merging strategy when a pipeline omits it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    merging: Option<MergingStrategyParams>,
    /// Default tiebreaker when a pipeline omits it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tiebreaker: Option<TiebreakerParams>,
    /// Default minimum confidence when a pipeline omits it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    min_confidence: Option<ConfidenceThreshold>,
}

impl EngineFile {
    /// Reads and parses the engine configuration from a TOML file.
    pub(super) async fn load(path: &Path) -> Result<Self> {
        let text = tokio::fs::read_to_string(path).await.map_err(|e| {
            Error::internal("engine", "Failed to read engine config file").with_source(e)
        })?;
        Self::parse(&text)
    }

    /// Parses the engine configuration from TOML text.
    pub(super) fn parse(text: &str) -> Result<Self> {
        toml::from_str(text).map_err(|e| {
            Error::internal("engine", "Failed to parse engine config file").with_source(e)
        })
    }

    /// Splits the file into the engine's recognizer lineups and the server-wide
    /// analyzer defaults derived from it.
    pub(super) fn into_parts(self) -> (NerConfig, LlmConfig, EngineDefaults) {
        let defaults = EngineDefaults {
            enrichers: self.enrichers,
            deduplication: DedupDefaults {
                calibration: self.deduplication.calibration,
                merging: self.deduplication.merging,
                tiebreaker: self.deduplication.tiebreaker,
                min_confidence: self.deduplication.min_confidence,
            },
            has_ner: !self.recognizers.ner.is_empty(),
            has_llm: !self.recognizers.llm.is_empty(),
        };
        let ner = NerConfig {
            recognizers: self.recognizers.ner,
        };
        let llm = LlmConfig {
            recognizers: self.recognizers.llm,
        };
        (ner, llm, defaults)
    }
}

/// Server-wide analyzer defaults merged into every run's `AnalyzerParams`.
///
/// These are operator-owned and uniform across the deployment; a pipeline's
/// definition carries detection intent, not this infrastructure config.
#[derive(Debug, Clone, Default)]
pub struct EngineDefaults {
    /// Enrichment applied to every run.
    pub enrichers: EnricherParams,
    /// Deduplication defaults (calibration + per-field fallbacks).
    pub deduplication: DedupDefaults,
    /// Whether an NER lineup is configured (gates `recognizers.ner`).
    pub has_ner: bool,
    /// Whether an LLM lineup is configured (gates `recognizers.llm`).
    pub has_llm: bool,
}

/// Server-wide deduplication defaults.
///
/// `calibration` is operator-only; the other fields are the deployment
/// fallbacks a pipeline inherits per-field when it leaves them unset.
#[derive(Debug, Clone, Default)]
pub struct DedupDefaults {
    /// Per-recognizer confidence weights for the calibrate layer.
    pub calibration: HashMap<String, f64>,
    /// Default merging strategy for pipelines that omit it.
    pub merging: Option<MergingStrategyParams>,
    /// Default tiebreaker for pipelines that omit it.
    pub tiebreaker: Option<TiebreakerParams>,
    /// Default minimum confidence for pipelines that omit it.
    pub min_confidence: Option<ConfidenceThreshold>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The committed example config must stay parseable as the documented shape.
    #[test]
    fn example_config_parses() {
        let text = include_str!("../../../../../docker/engine.example.toml");
        let file = EngineFile::parse(text).expect("example engine config parses");
        assert_eq!(file.recognizers.ner.len(), 1);
        assert_eq!(file.recognizers.llm.len(), 1);
        assert!(file.enrichers.ocr.is_some());
        assert!(file.enrichers.stt.is_some());
        assert!(!file.deduplication.calibration.is_empty());
        assert!(file.deduplication.merging.is_some());
    }
}
