//! Commonly used items from nvisy-service.
//!
//! This prelude module exports the most commonly used types, traits, and services
//! to simplify imports in consuming code.
//!
//! # Usage
//!
//! ```rust,ignore
//! use nvisy_service::prelude::*;
//! ```

// Error types
// Inference types and traits
pub use crate::inference::{
    Annotation, AnnotationRelation, AnnotationSet, AnnotationType, BoundingBox, Chat, Content,
    Context, Document, DocumentId, DocumentMetadata, EmbeddingBatchRequest, EmbeddingBatchResponse,
    EmbeddingFormat, EmbeddingRequest, EmbeddingResponse, InferenceProvider, InferenceService,
    Message, MessageRole, OcrBatchRequest, OcrBatchResponse, OcrRequest, OcrResponse, RelationType,
    SharedContext, TextExtraction, TextSpan, UsageStats, VlmBatchRequest, VlmBatchResponse,
    VlmRequest, VlmResponse, VlmUsage,
};
// Mock providers (test-utils feature)
#[cfg(feature = "test-utils")]
#[cfg_attr(docsrs, doc(cfg(feature = "test-utils")))]
pub use crate::inference::{MockConfig, MockProvider};
// Common types
pub use crate::types::{ServiceHealth, ServiceStatus, Timing};
// Webhook types and traits
pub use crate::webhook::{
    WebhookContext, WebhookPayload, WebhookProvider, WebhookRequest, WebhookResponse,
    WebhookService,
};
pub use crate::{BoxedError, Error, ErrorKind, Result};
