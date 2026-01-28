#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod definition;
pub mod engine;
mod error;
pub mod graph;

pub use engine::{ConnectionRegistry, PgConnectionLoader, ProviderConnection};
pub use error::{Error, Result};

/// Tracing target for runtime operations.
pub const TRACING_TARGET: &str = "nvisy_runtime";
