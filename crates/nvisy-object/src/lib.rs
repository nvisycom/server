#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod client;
/// Error and error-kind types for object-store operations.
pub mod error;
/// Client trait and object storage providers.
pub mod providers;
