#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod agent;
mod error;
pub mod provider;
pub mod rag;

pub use error::{Error, Result};

/// Tracing target for the main library.
pub const TRACING_TARGET: &str = "nvisy_rig";
