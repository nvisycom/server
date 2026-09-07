#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod error;
pub mod health;
pub mod net;

pub use error::{BoxedError, Error, ErrorKind, Result};

/// Tracing target for core operations.
pub const TRACING_TARGET: &str = "nvisy_core";
