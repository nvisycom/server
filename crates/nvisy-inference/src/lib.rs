#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod embedding;
mod language;
mod optical;
mod service;
pub mod types;

// Re-export commonly used items at crate root for convenience
pub use embedding::{
    EmbeddingBatchRequest, EmbeddingBatchResponse, EmbeddingProvider, EmbeddingProviderExt,
    EmbeddingRequest, EmbeddingResponse,
};
pub use language::{
    LanguageProvider, LanguageProviderExt, VlmBatchRequest, VlmBatchResponse, VlmRequest,
    VlmResponse,
};
pub use nvisy_core::{Error, ErrorKind, Result};
pub use optical::{
    OcrBatchRequest, OcrBatchResponse, OcrRequest, OcrResponse, OpticalProvider,
    OpticalProviderExt, TextExtraction,
};
pub use service::{Context, InferenceProvider, InferenceService, UsageStats};

/// Tracing target for inference operations.
pub const TRACING_TARGET: &str = "nvisy_inference";
