//! Workflow execution engine.
//!
//! This module provides the runtime for executing workflows:
//! - [`Engine`]: The main execution engine
//! - [`EngineConfig`]: Configuration options
//! - [`ExecutionContext`]: Runtime context for workflow execution
//! - [`CredentialsRegistry`]: Registry for AI provider credentials

mod compiler;
mod config;
mod context;
mod credentials;
mod executor;

pub use config::EngineConfig;
pub use context::{Context, ExecutionContext};
pub use credentials::{CredentialsRegistry, ProviderCredentials};
pub use executor::Engine;
