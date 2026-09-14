//! The injectable redaction-engine service.

use std::collections::HashSet;

use derive_more::Deref;
use elide_pipeline::file::Document;
use elide_pipeline::governance::policy::Policy;
use elide_pipeline::provider::{ProviderConfig, RequestContext};
use elide_pipeline::{
    Analyzed, Engine, Error as EngineError, ErrorKind as EngineErrorKind, Result as EngineResult,
};

use super::config::{self, EngineConfig};
use super::error::UnknownFormatToken;
use crate::Result;

/// The redaction engine, injectable via [`State`].
///
/// Cheaply cloneable — the underlying [`Engine`] is `Arc`-backed, so every clone
/// shares one configured codec registry and recognizer lineup. Derefs to the
/// [`Engine`] so callers can `analyze_document` / `anonymize_document` directly.
///
/// [`State`]: axum::extract::State
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
    ///
    /// # Errors
    ///
    /// - A configuration error if a `config_path` is set but the provider config
    ///   file cannot be read or parsed.
    pub async fn from_config(config: EngineConfig) -> Result<Self> {
        let provider_config = match config.config_path {
            Some(path) => config::load(&path).await?,
            None => ProviderConfig::default(),
        };

        let engine = Engine::new(provider_config.build());
        Ok(Self { engine })
    }

    /// Borrows the underlying [`Engine`].
    #[must_use]
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// Runs [`analyze`] on a blocking thread.
    ///
    /// With the default (local) recognizer lineup, `analyze` is CPU-bound: its
    /// future does regex and language-detection work inline and never yields to
    /// the runtime, so awaiting it directly pins an async worker thread for the
    /// whole analysis and starves every other task on that thread — including the
    /// short DB reads that authorization and other handlers hold a pooled
    /// connection across, which is how a burst of detections exhausts the
    /// connection pool. Offloading to the blocking pool keeps that CPU off the
    /// async workers so the rest of the server keeps scheduling.
    ///
    /// The future is driven on the blocking thread with the current Tokio
    /// runtime's handle, so a model-backed lineup (whose recognizers genuinely
    /// await network I/O over the runtime's reactor) still makes progress; the
    /// default local lineup runs to completion on the first poll and never
    /// touches the reactor. Inputs are moved in by value (the [`Engine`] is
    /// `Arc`-backed, so the clone is cheap and shares one configured lineup).
    ///
    /// [`analyze`]: Engine::analyze
    ///
    /// # Errors
    ///
    /// - A `Processing` engine error if the blocking task panics or is cancelled.
    /// - Any engine error [`analyze`](Engine::analyze) itself produces.
    pub async fn analyze_blocking(
        &self,
        document: Document,
        policies: Vec<Policy>,
        request: RequestContext,
    ) -> EngineResult<Analyzed> {
        let engine = self.engine.clone();
        let handle = tokio::runtime::Handle::current();
        tokio::task::spawn_blocking(move || {
            handle.block_on(engine.analyze(document, &policies, &request))
        })
        .await
        .map_err(|err| EngineError::new(EngineErrorKind::Processing, err))?
    }

    /// Resolves file-extension filter tokens to the set of extensions to match.
    ///
    /// Each token is a file extension (`pdf`, `jpg`); it expands to its format's
    /// full extension set so siblings match too (e.g. `jpg` also matches
    /// `jpeg`). An unknown extension is returned as an error so the request
    /// rejects rather than silently matching nothing.
    ///
    /// Extensions are lowercased and de-duplicated, preserving first-seen order.
    ///
    /// # Errors
    ///
    /// - [`UnknownFormatToken::Extension`] if a token is not a known file
    ///   extension in the codec registry. Empty tokens are skipped, not rejected.
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
    ///
    /// # Errors
    ///
    /// - [`UnknownFormatToken::Modality`] if a token matches no format's modality.
    ///   Empty tokens are skipped, not rejected.
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
