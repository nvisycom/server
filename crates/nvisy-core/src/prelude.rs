//! Commonly used items from nvisy-core.
//!
//! This prelude module exports the most commonly used types, traits, and functions
//! from nvisy-core to simplify imports in consuming code.

pub use crate::AiServices;
pub use crate::emb::{
    ContentEmbeddingRequest, Context as EmbeddingContext, EmbeddingData, EmbeddingProvider,
    EmbeddingResult, EmbeddingService, EncodingFormat, Request as EmbeddingRequest,
    Response as EmbeddingResponse,
};
pub use crate::error::{Error, ErrorKind, Result};
pub use crate::ocr::{
    Context as OcrContext, OcrProvider, OcrService, Request as OcrRequest, Response as OcrResponse,
};
pub use crate::types::{
    Annotation, AnnotationType, BoundingBox, Chat, Document, Message, MessageRole, ServiceHealth,
    ServiceStatus,
};
pub use crate::vlm::{
    Context as VlmContext, ImageInput, ProcessingOptions, Request as VlmRequest,
    Response as VlmResponse, VlmProvider, VlmService,
};
