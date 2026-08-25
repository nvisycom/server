//! Redaction engine service.
//!
//! Wraps the runtime's [`Engine`] — the stateless detect/redact pipeline — as a
//! dependency-injectable service. The engine is configured once at startup with
//! the deployment's NER/LLM recognizer lineups and OCR/STT enricher backends;
//! each request then drives analyze / anonymize against it. Deduplication and
//! calibration are engine-owned solid defaults, not configured here.

use std::collections::HashSet;
use std::path::PathBuf;

use derive_more::Deref;
use elide_pipeline::{
    CodecParams, DocumentContext, Engine, ProviderConfig, RasterMode, RequestContext,
};

use crate::Result;
use crate::handler::request::PipelineDefinition;

mod config;
mod error;

pub use error::UnknownFormatToken;

/// Deployment configuration for the redaction engine.
#[must_use]
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "cli", derive(clap::Args))]
pub struct EngineConfig {
    /// Optional path to a TOML file with the deployment engine configuration.
    ///
    /// Carries the NER/LLM recognizer lineups and the OCR/STT enricher
    /// backends. Absent means no NER/LLM recognizers and no enrichment (pattern
    /// recognizers still run).
    #[cfg_attr(feature = "cli", arg(long, env = "ENGINE_CONFIG_FILEPATH"))]
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
}

impl EngineService {
    /// Builds the engine from the deployment configuration.
    ///
    /// Loads the NER/LLM lineups and the OCR/STT enricher backends from the
    /// configured file when present; otherwise starts with empty lineups and no
    /// enrichment.
    pub async fn from_config(config: EngineConfig) -> Result<Self> {
        let provider_config = match config.config_path {
            Some(path) => config::load(&path).await?,
            None => ProviderConfig::default(),
        };

        let engine = Engine::new(provider_config.build());
        Ok(Self { engine })
    }

    /// Borrows the underlying [`Engine`].
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// Builds the [`RequestContext`] for one detect run from a pipeline's intent.
    ///
    /// Recognition is entirely engine-owned (the built-in pattern set plus the
    /// deployment's NER/LLM lineups always run). The document context is the
    /// request's own, falling back to the pipeline default. `raster_mode` is the
    /// workspace's page-rasterisation policy (always render vs. auto), carried in
    /// the codec params since it is server-derived, not caller-set. The engine
    /// records the context and codec params on the audit so redaction re-decodes
    /// and re-compiles against exactly what detection used. Deduplication and
    /// calibration are engine-owned defaults; the label catalog is derived from
    /// the run's policies at detect time.
    ///
    /// No key is set: the server does not yet drive keyed operators
    /// (`HmacHash`/`Encrypt`), whose `KeyConfig` would be supplied here.
    pub fn request_context(
        &self,
        definition: &PipelineDefinition,
        request_context: Option<DocumentContext>,
        raster_mode: RasterMode,
    ) -> RequestContext {
        let context = request_context
            .or_else(|| definition.default_scope.clone())
            .unwrap_or_default();
        RequestContext::new()
            .with_context(context)
            .with_codec(CodecParams::new().with_raster_mode(raster_mode))
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

    /// Whether the engine has a codec registered for `extension`.
    ///
    /// The check is what decides if an uploaded file can be processed at all, so
    /// callers can reject an unsupported format at upload time rather than
    /// failing later when detection tries (and fails) to decode it. Matching is
    /// case-insensitive.
    #[must_use]
    pub fn supports_extension(&self, extension: &str) -> bool {
        self.engine
            .formats()
            .by_extension(&extension.trim().to_ascii_lowercase())
            .is_some()
    }
}

/// Pushes `ext` (lowercased) into `out` if not already present.
fn push_unique(out: &mut Vec<String>, seen: &mut HashSet<String>, ext: &str) {
    let ext = ext.to_ascii_lowercase();
    if seen.insert(ext.clone()) {
        out.push(ext);
    }
}
