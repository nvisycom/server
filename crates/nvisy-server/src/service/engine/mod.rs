//! Redaction engine service.
//!
//! Wraps the runtime's [`Engine`] — the stateless detect/redact pipeline — as a
//! dependency-injectable service. The engine is configured once at startup with
//! the deployment's NER/LLM recognizer lineups and the server-wide enrichment
//! and deduplication-calibration defaults; each request then drives analyze /
//! anonymize against it, merging those defaults with the pipeline's intent.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use derive_more::Deref;
use nvisy_engine::Engine;
use nvisy_engine::plan::{
    AnalyzerParams, AnyAnnotations, DeduplicationParams, ProviderSelection, ScopeParams,
};

use crate::Result;
use crate::handler::request::PipelineDefinition;
use crate::handler::{ErrorKind, Result as HandlerResult};

mod config;
mod error;

use config::{EngineDefaults, EngineFile};
pub use error::UnknownFormatToken;

/// Deployment configuration for the redaction engine.
#[must_use]
#[derive(Debug, Clone, Default)]
pub struct EngineConfig {
    /// Optional path to a TOML file with the deployment engine configuration.
    ///
    /// Carries the NER/LLM recognizer lineups plus the server-wide enrichment
    /// and deduplication-calibration defaults. Absent means no NER/LLM
    /// recognizers, no enrichment, and no calibration (pattern recognizers
    /// still run).
    pub config_path: Option<PathBuf>,
}

/// The redaction engine, injectable via [`State`](axum::extract::State).
///
/// Cheaply cloneable — the underlying [`Engine`] is `Arc`-backed, so every clone
/// shares one configured codec registry and recognizer lineup. Derefs to the
/// [`Engine`] so callers can `analyze_document` / `anonymize_document` directly.
#[derive(Clone, Deref)]
#[must_use = "the engine does nothing unless you analyze or anonymize with it"]
pub struct EngineService {
    #[deref]
    engine: Engine,
    defaults: Arc<EngineDefaults>,
}

impl EngineService {
    /// Builds the engine from the deployment configuration.
    ///
    /// Loads the NER/LLM lineups and the server-wide enrichment / calibration
    /// defaults from the configured file when present; otherwise starts with
    /// empty lineups, no enrichment, and no calibration.
    pub async fn from_config(config: EngineConfig) -> Result<Self> {
        let file = match config.config_path {
            Some(path) => EngineFile::load(&path).await?,
            None => EngineFile::default(),
        };

        let (ner, llm, defaults) = file.into_parts();
        let engine = Engine::new().with_ner(ner).with_llm(llm);
        Ok(Self {
            engine,
            defaults: Arc::new(defaults),
        })
    }

    /// Borrows the underlying [`Engine`].
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// Builds the [`AnalyzerParams`] for one detect run by merging a pipeline's
    /// intent with the deployment defaults.
    ///
    /// Recognizers and deduplication behavior come from the pipeline; enrichment
    /// and deduplication calibration come from the server-wide defaults. Scope is
    /// the request's own, falling back to the pipeline default. The label catalog
    /// is not set here: the engine derives it from the run's policies at detect
    /// time.
    ///
    /// Rejects a pipeline that explicitly enables NER or LLM recognizers this
    /// deployment has no lineup for, rather than silently running without them.
    pub fn analyzer_params(
        &self,
        definition: &PipelineDefinition,
        request_scope: Option<ScopeParams>,
    ) -> HandlerResult<AnalyzerParams> {
        let recognizers = &definition.recognizers;
        if wants_recognizer(recognizers.ner.as_ref()) && !self.defaults.has_ner {
            return Err(ErrorKind::BadRequest
                .with_message("NER recognition is not available in this deployment")
                .with_resource("pipeline"));
        }
        if wants_recognizer(recognizers.llm.as_ref()) && !self.defaults.has_llm {
            return Err(ErrorKind::BadRequest
                .with_message("LLM recognition is not available in this deployment")
                .with_resource("pipeline"));
        }

        let scope = request_scope
            .or_else(|| definition.default_scope.clone())
            .unwrap_or_default();

        // Deduplication: a pipeline field wins; otherwise the deployment default;
        // otherwise the engine baseline. Calibration is operator-only.
        let dedup = &definition.deduplication;
        let dedup_defaults = &self.defaults.deduplication;
        let deduplication = DeduplicationParams {
            calibration: dedup_defaults.calibration.clone(),
            merging: dedup.merging.or(dedup_defaults.merging).unwrap_or_default(),
            tiebreaker: dedup
                .tiebreaker
                .or(dedup_defaults.tiebreaker)
                .unwrap_or_default(),
            min_confidence: dedup.min_confidence.or(dedup_defaults.min_confidence),
        };

        Ok(AnalyzerParams {
            recognizers: recognizers.clone(),
            enrichers: self.defaults.enrichers.clone(),
            deduplication,
            scope,
            annotations: AnyAnnotations::default(),
        })
    }

    /// Resolves file-extension filter tokens to the set of extensions to match.
    ///
    /// Each token is a file extension (`pdf`, `jpg`); it expands to its format's
    /// full extension set so siblings match too (e.g. `jpg` also matches
    /// `jpeg`). An unknown extension is returned as an error so the request
    /// rejects rather than silently matching nothing.
    ///
    /// Extensions are lowercased and de-duplicated, preserving first-seen order.
    pub fn resolve_extensions<I, S>(&self, tokens: I) -> Result<Vec<String>, UnknownFormatToken>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let registry = self.engine.formats();
        let mut out = Vec::new();
        let mut seen = HashSet::new();

        for token in tokens {
            let token = token.as_ref().trim().to_ascii_lowercase();
            if token.is_empty() {
                continue;
            }
            match registry.by_extension(&token) {
                Some(format) => {
                    for ext in format.extensions() {
                        push_unique(&mut out, &mut seen, ext.as_ref());
                    }
                }
                None => return Err(UnknownFormatToken::Extension(token)),
            }
        }

        Ok(out)
    }

    /// Resolves modality keywords (`text`, `tabular`, `image`, `audio`) to the
    /// set of file extensions of every format of those modalities.
    ///
    /// An unknown modality is returned as an error. Extensions are lowercased
    /// and de-duplicated, preserving first-seen order.
    pub fn resolve_modalities<I, S>(&self, tokens: I) -> Result<Vec<String>, UnknownFormatToken>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let registry = self.engine.formats();
        let mut out = Vec::new();
        let mut seen = HashSet::new();

        for token in tokens {
            let token = token.as_ref().trim().to_ascii_lowercase();
            if token.is_empty() {
                continue;
            }
            let mut matched = false;
            for format in registry.iter() {
                if format.modality() == token {
                    matched = true;
                    for ext in format.extensions() {
                        push_unique(&mut out, &mut seen, ext.as_ref());
                    }
                }
            }
            if !matched {
                return Err(UnknownFormatToken::Modality(token));
            }
        }

        Ok(out)
    }
}

/// Whether a provider selection is an explicit request to run recognizers
/// (`All(true)` or a non-empty `Only` allowlist), as opposed to off or the
/// softly-on default.
fn wants_recognizer(selection: Option<&ProviderSelection>) -> bool {
    match selection {
        Some(ProviderSelection::All(enabled)) => *enabled,
        Some(ProviderSelection::Only(names)) => !names.is_empty(),
        None => false,
    }
}

/// Pushes `ext` (lowercased) into `out` if not already present.
fn push_unique(out: &mut Vec<String>, seen: &mut HashSet<String>, ext: &str) {
    let ext = ext.to_ascii_lowercase();
    if seen.insert(ext.clone()) {
        out.push(ext);
    }
}
