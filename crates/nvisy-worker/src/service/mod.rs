//! Worker state and configuration.
//!
//! This module provides the core state management for document processing workers,
//! including configuration for concurrency limits and connections to external services.

mod config;
mod state;

pub use config::WorkerConfig;
pub use state::WorkerState;
