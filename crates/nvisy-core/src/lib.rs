#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod emb;
mod error;
#[cfg(feature = "test-utils")]
mod mock;
pub mod ocr;
#[doc(hidden)]
pub mod prelude;
mod services;
mod types;
pub mod vlm;

pub use error::{BoxedError, Error, ErrorKind, Result};
#[cfg(feature = "test-utils")]
#[cfg_attr(docsrs, doc(cfg(feature = "test-utils")))]
pub use mock::{MockConfig, MockProvider};
pub use services::AiServices;
pub use types::{
    Annotation, AnnotationRelation, AnnotationSet, AnnotationType, BoundingBox, Chat, Content,
    Context, Document, DocumentId, DocumentMetadata, Message, MessageRole, RelationType,
    ServiceHealth, ServiceStatus, SharedContext, TextSpan, Timing, UsageStats,
};
