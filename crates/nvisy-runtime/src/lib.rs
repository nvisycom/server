#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod engine;
mod error;
pub mod graph;
pub mod node;
pub mod runtime;

#[doc(hidden)]
pub mod prelude;

pub use error::{WorkflowError, WorkflowResult};

/// Tracing target for runtime operations.
pub const TRACING_TARGET: &str = "nvisy_runtime";
