#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod emb;
mod error;
pub mod ocr;
#[doc(hidden)]
pub mod prelude;
mod types;
pub mod vlm;

pub use error::{BoxedError, Error, ErrorKind, Result};
pub use types::{
    Annotation, AnnotationRelation, AnnotationSet, AnnotationType, BoundingBox, Chat, Content,
    Document, DocumentId, DocumentMetadata, Message, MessageRole, RelationType, ServiceHealth,
    ServiceStatus, TextSpan,
};
