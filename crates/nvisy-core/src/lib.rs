#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod health;

/// Tracing target for core operations.
pub const TRACING_TARGET: &str = "nvisy_core";
