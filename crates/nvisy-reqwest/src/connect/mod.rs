//! Reqwest client module.
//!
//! This module provides the main client interface for HTTP operations.
//! It wraps the `reqwest` crate for webhook delivery.

mod client;
mod config;

pub use client::{ReqwestClient, TRACING_TARGET};
pub use config::ReqwestConfig;
