//! Deployment engine configuration file.
//!
//! The operator-owned TOML file (pointed at by `ENGINE_CONFIG_FILEPATH`) that
//! configures the engine at startup: the NER/LLM recognizer lineups and the
//! OCR/STT enricher backends. These are uniform across the deployment; a
//! pipeline's definition carries detection intent, not this infrastructure
//! config.

use std::path::Path;

use nvisy_engine::provider::{
    LlmConfig, LlmRecognizerConfig, NerConfig, NerRecognizerConfig, OcrBackend, SttBackend,
};
use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// The deployment engine configuration, as loaded from the config file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct EngineFile {
    /// Deployment recognizer lineups, keyed by kind (`recognizers.ner`,
    /// `recognizers.llm`).
    #[serde(default)]
    recognizers: EngineRecognizers,
    /// Enricher backends applied uniformly to every run.
    #[serde(default)]
    enrichers: EngineEnrichers,
}

/// The deployment's enricher backends.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EngineEnrichers {
    /// OCR enricher backend (image modality). Absent means no OCR.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ocr: Option<OcrBackendFile>,
    /// STT enricher backend (audio modality). Absent means no STT.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    stt: Option<SttBackendFile>,
}

/// The deployment's recognizer lineups.
///
/// Each is a flat list of recognizer instances the operator wired up; every
/// entry runs on every request whose modality matches.
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

/// Deserializable mirror of [`OcrBackend`], which is `#[non_exhaustive]` and not
/// itself `Deserialize`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum OcrBackendFile {
    /// BentoML-hosted OCR service.
    Bento {
        /// Base URL of the BentoML service.
        base_url: String,
        /// Model identifier the backend should target.
        model: String,
    },
}

impl From<OcrBackendFile> for OcrBackend {
    fn from(file: OcrBackendFile) -> Self {
        match file {
            OcrBackendFile::Bento { base_url, model } => OcrBackend::Bento { base_url, model },
        }
    }
}

/// Deserializable mirror of [`SttBackend`], which is `#[non_exhaustive]` and not
/// itself `Deserialize`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum SttBackendFile {
    /// BentoML-hosted STT service.
    Bento {
        /// Base URL of the BentoML service.
        base_url: String,
        /// Model identifier the backend should target.
        model: String,
    },
}

impl From<SttBackendFile> for SttBackend {
    fn from(file: SttBackendFile) -> Self {
        match file {
            SttBackendFile::Bento { base_url, model } => SttBackend::Bento { base_url, model },
        }
    }
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

    /// Splits the file into the engine's recognizer lineups and enricher
    /// backends, ready to build the engine with.
    pub(super) fn into_parts(self) -> EngineParts {
        EngineParts {
            ner: NerConfig {
                recognizers: self.recognizers.ner,
            },
            llm: LlmConfig {
                recognizers: self.recognizers.llm,
            },
            ocr: self.enrichers.ocr.map(OcrBackend::from),
            stt: self.enrichers.stt.map(SttBackend::from),
        }
    }
}

/// The engine-building inputs derived from an [`EngineFile`].
pub(super) struct EngineParts {
    /// NER recognizer lineup.
    pub(super) ner: NerConfig,
    /// LLM recognizer lineup.
    pub(super) llm: LlmConfig,
    /// OCR enricher backend, when configured.
    pub(super) ocr: Option<OcrBackend>,
    /// STT enricher backend, when configured.
    pub(super) stt: Option<SttBackend>,
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
    }
}
