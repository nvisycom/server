#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

#[cfg(feature = "cli")]
mod duration;
mod error;
pub mod health;
pub mod net;

#[cfg(feature = "cli")]
pub use duration::parse_duration;
pub use error::{BoxedError, Error, ErrorKind, Result};

/// Tracing target for core operations.
pub const TRACING_TARGET: &str = "nvisy_core";
