#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

//! # nvisy-ollama
//!
//! Ollama client library for nvisy, providing embeddings, OCR, and VLM capabilities.
//!
//! The [`OllamaClient`] implements the [`InferenceProvider`] trait from nvisy-service.
//!
//! [`InferenceProvider`]: nvisy_service::InferenceProvider

/// Tracing target for the main library.
pub const TRACING_TARGET: &str = "nvisy_ollama";

/// Tracing target for client operations.
pub const TRACING_TARGET_CLIENT: &str = "nvisy_ollama::client";

mod client;
mod error;
mod provider;

pub use crate::client::{OllamaClient, OllamaConfig};
pub use crate::error::{Error, Result};
