//! Workflow execution engine.
//!
//! This module provides the runtime for executing workflows:
//! - [`Engine`]: The main execution engine
//! - [`EngineConfig`]: Configuration options
//! - [`ExecutionContext`]: Runtime context for workflow execution

mod config;
mod context;
mod executor;

pub use config::EngineConfig;
pub use context::ExecutionContext;
pub use executor::Engine;
