//! Commonly used items from nvisy-core.
//!
//! This prelude module exports the most commonly used traits and the main
//! service container to simplify imports in consuming code.

pub use crate::AiServices;
pub use crate::emb::EmbeddingProvider;
pub use crate::error::{Error, ErrorKind, Result};
pub use crate::ocr::OcrProvider;
pub use crate::vlm::VlmProvider;
