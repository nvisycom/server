#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

//! # Nvisy Core
//!
//! This crate provides the foundational abstractions for AI services in the Nvisy ecosystem.
//! It defines core traits and types for Vision Language Models (VLMs) and Optical Character
//! Recognition (OCR) services without depending on any concrete implementations.

/// Tracing target for OCR operations.
pub const TRACING_TARGET_OCR: &str = "nvisy_core::ocr";

/// Tracing target for VLM operations.
pub const TRACING_TARGET_VLM: &str = "nvisy_core::vlm";

mod error;
mod health;

pub mod ocr;
pub mod vlm;

// Re-export key types for convenience
pub use error::BoxedError;
pub use health::{ServiceHealth, ServiceStatus};
