//! Commonly used items from nvisy-core.
//!
//! This prelude module exports the most commonly used types, traits, and functions
//! from nvisy-core to simplify imports in consuming code.

pub use crate::emb::{
    Context as EmbeddingContext, EmbeddingProvider, EmbeddingRequest, EmbeddingResponse,
    EmbeddingService, EncodingFormat,
};
pub use crate::error::{Error, ErrorKind, Result};
pub use crate::ocr::{
    Context as OcrContext, OcrProvider, OcrService, Request as OcrRequest, Response as OcrResponse,
};
pub use crate::types::{
    Annotation, AnnotationType, BoundingBox, Chat, Document, Message, MessageRole, ServiceHealth,
    ServiceStatus,
};
