//! Deployment engine configuration file.
//!
//! The operator-owned TOML file (pointed at by `ENGINE_CONFIG_FILEPATH`) that
//! configures the engine at startup: the NER/LLM recognizer lineups and the
//! OCR/STT enricher backends. These are uniform across the deployment; a
//! pipeline's definition carries detection intent, not this infrastructure
//! config.

use std::path::Path;

use nvisy_engine::provider::{
    LlmConfig, LlmRecognizerConfig, NerConfig, NerRecognizerConfig, OcrConfig, OcrEnricherConfig,
    SttConfig, SttEnricherConfig,
};
use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// The deployment engine configuration, as loaded from the config file.
///
/// Each lineup entry deserializes straight into the engine's own config element
/// type, so the file needs no server-side mirror types.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct EngineFile {
    /// Deployment recognizer lineups (`recognizers.ner`, `recognizers.llm`).
    #[serde(default)]
    recognizers: EngineRecognizers,
    /// Enricher backends applied uniformly to every run (`enrichers.ocr`,
    /// `enrichers.stt`).
    #[serde(default)]
    enrichers: EngineEnrichers,
}

/// The deployment's recognizer lineups.
///
/// Each entry runs on every request whose modality matches.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EngineRecognizers {
    /// NER recognizer lineup.
    #[serde(default)]
    ner: Vec<NerRecognizerConfig>,
    /// LLM recognizer lineup.
    #[serde(default)]
    llm: Vec<LlmRecognizerConfig>,
}

/// The deployment's enricher lineups.
///
/// At most one enricher attaches per modality; the wire keeps a list for
/// symmetry with the recognizer lineups.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EngineEnrichers {
    /// OCR enricher lineup (image modality). Empty means no OCR.
    #[serde(default)]
    ocr: Vec<OcrEnricherConfig>,
    /// STT enricher lineup (audio modality). Empty means no STT.
    #[serde(default)]
    stt: Vec<SttEnricherConfig>,
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

    /// Wraps the parsed lineups into the engine's config types, ready to build
    /// the engine with.
    pub(super) fn into_parts(self) -> EngineParts {
        EngineParts {
            ner: NerConfig {
                recognizers: self.recognizers.ner,
            },
            llm: LlmConfig {
                recognizers: self.recognizers.llm,
            },
            ocr: OcrConfig {
                enrichers: self.enrichers.ocr,
            },
            stt: SttConfig {
                enrichers: self.enrichers.stt,
            },
        }
    }
}

/// The engine-building inputs derived from an [`EngineFile`].
pub(super) struct EngineParts {
    /// NER recognizer lineup.
    pub(super) ner: NerConfig,
    /// LLM recognizer lineup.
    pub(super) llm: LlmConfig,
    /// OCR enricher lineup.
    pub(super) ocr: OcrConfig,
    /// STT enricher lineup.
    pub(super) stt: SttConfig,
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
        assert_eq!(file.enrichers.ocr.len(), 1);
        assert_eq!(file.enrichers.stt.len(), 1);
    }
}
