#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod client;
/// Provider trait and object storage provider factories.
pub mod providers;
/// Streaming traits and object store adapters.
pub mod streams;
/// Inlined types (Error, ContentData, ContentSource).
pub mod types;

#[doc(hidden)]
pub mod prelude;
