#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod error;
pub mod inference;
#[doc(hidden)]
pub mod prelude;
mod types;
pub mod webhook;

pub use error::{BoxedError, Error, ErrorKind, Result};
pub use inference::{InferenceProvider, InferenceService};
#[cfg(feature = "test-utils")]
#[cfg_attr(docsrs, doc(cfg(feature = "test-utils")))]
pub use inference::{MockConfig, MockProvider};
pub use types::{
    Annotation, AnnotationRelation, AnnotationSet, AnnotationType, BoundingBox, Chat, Content,
    Context, Document, DocumentId, DocumentMetadata, Message, MessageRole, RelationType,
    ServiceHealth, ServiceStatus, SharedContext, TextSpan, Timing, UsageStats,
};
