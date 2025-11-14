//! PaddleX HTTP client module.
//!
//! This module provides the client interface for interacting with PaddleX services,
//! including PaddleOCR-VL for document parsing.

mod pd_client;
mod pd_config;

pub use pd_client::PdClient;
pub use pd_config::PdConfig;
