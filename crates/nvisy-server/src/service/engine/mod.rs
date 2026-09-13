//! Redaction engine service.
//!
//! Wraps the runtime's [`Engine`](elide_pipeline::Engine) — the stateless
//! detect/redact pipeline — as a dependency-injectable service. The engine is
//! configured once at startup with the deployment's NER/LLM recognizer lineups
//! and OCR/STT enricher backends; each request then drives analyze / anonymize
//! against it. Deduplication and calibration are engine-owned solid defaults, not
//! configured here.

mod config;
mod error;
mod service;

pub use config::EngineConfig;
pub use error::UnknownFormatToken;
pub use service::EngineService;
