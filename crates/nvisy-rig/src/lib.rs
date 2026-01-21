#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod agent;
pub mod chat;
mod error;
pub mod provider;
pub mod rag;
mod service;
mod session;
mod tool;

pub use error::{Error, Result};
pub use service::{RigConfig, RigService};

/// Tracing target for the main library.
pub const TRACING_TARGET: &str = "nvisy_rig";
