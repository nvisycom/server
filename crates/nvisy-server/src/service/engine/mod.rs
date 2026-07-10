//! Redaction engine service.
//!
//! Wraps the runtime's [`Engine`] — the stateless detect/redact pipeline — as a
//! dependency-injectable service. The engine is configured once at startup with
//! the deployment's NER and LLM recognizer lineups; each request then drives
//! analyze / anonymize against it.

use std::path::PathBuf;

use derive_more::Deref;
use nvisy_engine::Engine;
use nvisy_engine_core::llm::LlmConfig;
use nvisy_engine_core::ner::NerConfig;
use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// Deployment configuration for the redaction engine.
#[must_use]
#[derive(Debug, Clone, Default)]
pub struct EngineConfig {
    /// Optional path to a JSON file with the NER/LLM recognizer lineups.
    ///
    /// Absent means no NER/LLM recognizers are configured (pattern recognizers
    /// still run); the inference-backed lineups arrive with the sidecars.
    pub config_path: Option<PathBuf>,
}

/// The recognizer lineups the engine is built with, as loaded from config.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecognizerLineups {
    /// NER recognizer lineup (runs when a request enables `recognizers.ner`).
    #[serde(default)]
    ner: NerConfig,
    /// LLM recognizer lineup (runs when a request enables `recognizers.llm`).
    #[serde(default)]
    llm: LlmConfig,
}

/// The redaction engine, injectable via [`State`](axum::extract::State).
///
/// Cheaply cloneable — the underlying [`Engine`] is `Arc`-backed, so every clone
/// shares one configured codec registry and recognizer lineup. Derefs to the
/// [`Engine`] so callers can `analyze_document` / `anonymize_document` directly.
#[derive(Clone, Deref)]
#[must_use = "the engine does nothing unless you analyze or anonymize with it"]
pub struct EngineService {
    engine: Engine,
}

impl EngineService {
    /// Builds the engine from the deployment configuration.
    ///
    /// Loads the recognizer lineups from the configured file when present;
    /// otherwise starts with empty NER/LLM lineups.
    pub async fn from_config(config: EngineConfig) -> Result<Self> {
        let lineups = match config.config_path {
            Some(path) => load_lineups(&path).await?,
            None => RecognizerLineups::default(),
        };
        let engine = Engine::new().with_ner(lineups.ner).with_llm(lineups.llm);
        Ok(Self { engine })
    }

    /// Borrows the underlying [`Engine`].
    pub fn engine(&self) -> &Engine {
        &self.engine
    }
}

/// Reads and parses the recognizer lineups from a JSON config file.
async fn load_lineups(path: &PathBuf) -> Result<RecognizerLineups> {
    let bytes = tokio::fs::read(path).await.map_err(|e| {
        Error::internal("engine", "Failed to read engine config file").with_source(e)
    })?;
    serde_json::from_slice(&bytes)
        .map_err(|e| Error::internal("engine", "Failed to parse engine config file").with_source(e))
}
