//! Workflow execution engine.
//!
//! This module provides the runtime for executing workflows:
//! - [`Engine`]: The main execution engine
//! - [`EngineConfig`]: Configuration options

mod config;
mod executor;

pub use config::EngineConfig;
pub use executor::Engine;
