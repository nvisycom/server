//! Deployment engine configuration file.
//!
//! The operator-owned TOML file (pointed at by `ENGINE_CONFIG_FILEPATH`) that
//! configures the engine at startup: the NER/LLM recognizer lineups and the
//! OCR/STT enricher backends. These are uniform across the deployment; a
//! pipeline's definition carries detection intent, not this infrastructure
//! config.
//!
//! The file deserializes straight into the engine's own [`ProviderConfig`], whose
//! `recognizers`/`enrichers` shape the file mirrors, so no server-side mirror
//! types are needed.

use std::path::Path;

use elide_pipeline::ProviderConfig;

use crate::{Error, Result};

/// Reads and parses the engine [`ProviderConfig`] from a TOML file.
pub(super) async fn load(path: &Path) -> Result<ProviderConfig> {
    let text = tokio::fs::read_to_string(path).await.map_err(|e| {
        Error::internal("engine", "Failed to read engine config file").with_source(e)
    })?;
    parse(&text)
}

/// Parses the engine [`ProviderConfig`] from TOML text.
pub(super) fn parse(text: &str) -> Result<ProviderConfig> {
    toml::from_str(text)
        .map_err(|e| Error::internal("engine", "Failed to parse engine config file").with_source(e))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The committed example config must stay parseable as the documented shape.
    #[test]
    fn example_config_parses() {
        let text = include_str!("../../../../../docker/engine.example.toml");
        let config = parse(text).expect("example engine config parses");
        assert_eq!(config.recognizers.ner.len(), 1);
        assert_eq!(config.recognizers.llm.len(), 1);
        assert_eq!(config.enrichers.ocr.len(), 1);
        assert_eq!(config.enrichers.stt.len(), 1);
    }
}
