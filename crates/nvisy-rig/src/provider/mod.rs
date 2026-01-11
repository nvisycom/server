//! Multi-provider management for AI inference.
//!
//! This module provides:
//! - [`ProviderRegistry`] - Registry of configured providers
//! - [`ProviderConfig`] - Configuration for individual providers
//! - [`ModelRef`] - Reference to a specific model (provider/model)
//! - [`EmbeddingProvider`] - Unified embedding provider enum

mod config;
mod embedding;
mod registry;

pub use config::{ModelConfig, ProviderConfig, ProviderKind};
pub use embedding::EmbeddingProvider;
pub use registry::{ModelRef, ProviderRegistry};
