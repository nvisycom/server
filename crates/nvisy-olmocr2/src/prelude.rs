//! Prelude for the nvisy-olmocr2 crate
//!
//! This module re-exports the most commonly used types and traits from the crate
//! to provide a convenient single import for users.
//!
//! # Examples
//!
//! ```rust
//! use nvisy_olmocr2::prelude::*;
//!
//! // Now you have access to all commonly used types
//! let client = OcrClient::new(config).await?;
//! let result = client.process_image(image_data).await?;
//! ```

// Re-export core types from lib.rs
pub use std::time::Duration;

// Re-export commonly used types from modules
// (These will be uncommented as modules are implemented)

// pub use crate::models::{
//     OlmoModel, ModelConfig, ModelCapabilities, SupportedLanguage,
// };

// pub use crate::processing::{
//     DocumentProcessor, ProcessingOptions, ImagePreprocessor,
//     BatchProcessor, ProcessingResult,
// };

// pub use crate::results::{
//     OcrResult, TextBlock, BoundingBox, ConfidenceScore,
//     DocumentMetadata, ExtractedText,
// };

// Re-export commonly used external types for convenience
pub use bytes::Bytes;
pub use mime::Mime;
pub use serde::{Deserialize, Serialize};
pub use tokio::time::timeout;
pub use url::Url;
pub use uuid::Uuid;

// Re-export client types
pub use crate::client::{OcrClient, OcrConfig, OcrCredentials};
pub use crate::{Error, Result};
// Re-export tracing targets for easy access
pub use crate::{
    TRACING_TARGET_API, TRACING_TARGET_CLIENT, TRACING_TARGET_MODELS, TRACING_TARGET_PROCESSING,
    TRACING_TARGET_RESULTS,
};
