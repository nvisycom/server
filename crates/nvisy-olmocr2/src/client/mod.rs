//! OCR client module
//!
//! This module provides the main client interface for OCR operations using OLMo v2 models.
//! It handles authentication, request/response processing, and connection management.

mod credentials;
mod olm_client;
mod olm_config;

pub use credentials::OlemCredentials;
pub use olm_client::OlmClient;
pub use olm_config::{OlmBuilder, OlmBuilderError, OlmConfig};
