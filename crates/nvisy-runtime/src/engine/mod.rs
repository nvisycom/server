//! Workflow execution engine.
//!
//! This module provides the runtime for executing workflows:
//! - [`Engine`]: The main execution engine
//! - [`EngineConfig`]: Configuration options
//! - [`ExecutionContext`]: Runtime context for workflow execution
//! - [`ConnectionRegistry`]: Registry for provider connections

mod compiler;
mod config;
mod connection;
mod context;
mod executor;

pub use config::EngineConfig;
pub use connection::{ConnectionRegistry, PgConnectionLoader, ProviderConnection};
pub use context::ExecutionContext;
pub use executor::Engine;
pub use nvisy_dal::contexts::AnyContext;
